//! Everything related to theme loading and execution by backends.

mod primitive_detection;

use std::{collections::HashMap, path::Path};

use erebus_theme::{OwnedManifest, settings::SettingValue};

use wasmtime::{
    Config, Engine, Instance, Linker, Memory, Module, Store, TypedFunc, ensure, format_err,
};

pub use wasmtime::Result;

pub use crate::primitive_detection::PrimitiveKind;

const MAX_MAILBOX_CAPACITY: u32 = 4096;
const DEFAULT_FUEL_PER_CALL: u64 = 100_000_000;

/// A compiled theme module.
#[derive(Clone)]
pub struct ThemeModule {
    engine: Engine,
    module: Module,
    fuel_per_call: u64,
}

impl ThemeModule {
    /// Loads and compiles a theme from a wasm/wat file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .map_err(|err| format_err!("failed to read theme file at {}: {err}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    /// Compiles a theme from .wasm bytes or .wat text.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);

        let engine = Engine::new(&config)?;
        let module = Module::new(&engine, bytes)?;

        Ok(Self {
            engine,
            module,
            fuel_per_call: DEFAULT_FUEL_PER_CALL,
        })
    }

    /// Overrides how much fuel each call into the module may burn.
    pub fn with_fuel_per_call(mut self, fuel: u64) -> Self {
        self.fuel_per_call = fuel;
        self
    }

    /// Creates a fresh, independent instance of the theme.
    pub fn instantiate(&self) -> Result<ThemeCtx> {
        ThemeCtx::new(self)
    }

    /// Reads the theme's manifest out of a throwaway instance.
    pub fn manifest(&self) -> Result<OwnedManifest> {
        let mut ctx = self.instantiate()?;
        ctx.check_abi_version()?;

        let manifest = ctx
            .instance
            .get_typed_func::<(), u64>(&mut ctx.store, "manifest")?;

        ctx.store.set_fuel(ctx.fuel_per_call)?;
        let packed = manifest.call(&mut ctx.store, ())?;
        let (ptr, len) = ((packed >> 32) as u32, packed as u32);
        ensure!(ptr != 0, "theme failed to serialize its manifest");

        let mut bytes = vec![0; len as usize];
        ctx.memory.read(&ctx.store, ptr as usize, &mut bytes)?;
        ctx.dealloc.call(&mut ctx.store, (ptr, len))?;

        Ok(postcard::from_bytes(&bytes)?)
    }
}

/// Live instance of a theme module once loaded.
pub struct ThemeCtx {
    pub instance: Instance,
    pub store: Store<()>,
    pub memory: Memory,
    pub alloc: TypedFunc<u32, u32>,
    pub dealloc: TypedFunc<(u32, u32), ()>,
    pub theme_quad: TypedFunc<(u32, u32), u32>,
    pub theme_shadow: TypedFunc<(u32, u32), u32>,
    pub theme_underline: TypedFunc<(u32, u32), u32>,
    pub theme_monochrome_sprite: TypedFunc<(u32, u32), u32>,
    pub theme_polychrome_sprite: TypedFunc<(u32, u32), u32>,
    pub theme_subpixel_sprite: TypedFunc<(u32, u32), u32>,

    fuel_per_call: u64,

    /// `(ptr, capacity)` of the block in module memory that uploads get copied to.
    ///
    /// This is reused between calls and only reallocated when an upload doesn't fit.
    mailbox_buffer: Option<(u32, u32)>,

    /// set after the module traps or misbehaves
    disabled: bool,
}

impl ThemeCtx {
    pub fn new(theme: &ThemeModule) -> Result<Self> {
        let mut store = Store::new(&theme.engine, ());
        store.set_fuel(theme.fuel_per_call)?; // give some fuel for any initialization we add in the future

        let instance = Linker::new(&theme.engine).instantiate(&mut store, &theme.module)?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| format_err!("theme module does not export `memory`"))?;

        Ok(Self {
            memory,
            alloc: instance.get_typed_func(&mut store, "alloc")?,
            dealloc: instance.get_typed_func(&mut store, "dealloc")?,
            theme_quad: instance.get_typed_func(&mut store, "theme_quad")?,
            theme_shadow: instance.get_typed_func(&mut store, "theme_shadow")?,
            theme_underline: instance.get_typed_func(&mut store, "theme_underline")?,
            theme_monochrome_sprite: instance
                .get_typed_func(&mut store, "theme_monochrome_sprite")?,
            theme_polychrome_sprite: instance
                .get_typed_func(&mut store, "theme_polychrome_sprite")?,
            theme_subpixel_sprite: instance.get_typed_func(&mut store, "theme_subpixel_sprite")?,
            instance,
            store,
            fuel_per_call: theme.fuel_per_call,
            mailbox_buffer: None,
            disabled: false,
        })
    }

    fn check_abi_version(&mut self) -> Result<()> {
        let abi_version = self
            .instance
            .get_typed_func::<(), u32>(&mut self.store, "abi_version")
            .map_err(|err| {
                format_err!(
                    "theme does not export a usable `abi_version` \
                     (was it built against an older erebus-theme?): {err}"
                )
            })?;

        self.store.set_fuel(self.fuel_per_call)?;
        let version = abi_version.call(&mut self.store, ())?;
        ensure!(
            version == erebus_theme::abi::ABI_VERSION,
            "theme was built for ABI version {version}, but this backend supports version {}",
            erebus_theme::abi::ABI_VERSION
        );

        Ok(())
    }

    /// Initializes the theme with `settings`. Missing or invalid values fall back to defaults.
    pub fn init(&mut self, settings: &HashMap<String, SettingValue>) -> Result<()> {
        self.check_abi_version()?;

        let init = self
            .instance
            .get_typed_func::<(u32, u32), ()>(&mut self.store, "init")?;

        let bytes = postcard::to_allocvec(settings)?;
        let len = u32::try_from(bytes.len())?;

        self.store.set_fuel(self.fuel_per_call)?;
        let ptr = self.reserve(len)?;
        self.memory.write(&mut self.store, ptr as usize, &bytes)?;
        init.call(&mut self.store, (ptr, len))?;

        Ok(())
    }

    /// Returns `true` if the module failed and was disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Ensures the mailbox can hold `len` bytes and returns its address in module memory.
    fn reserve(&mut self, len: u32) -> Result<u32> {
        if let Some((ptr, cap)) = self.mailbox_buffer {
            if cap >= len {
                return Ok(ptr);
            }

            // too small. Release it before asking for a bigger one
            self.mailbox_buffer = None;
            self.dealloc.call(&mut self.store, (ptr, cap))?;
        }

        let cap = len
            .max(MAX_MAILBOX_CAPACITY)
            .checked_next_power_of_two()
            .ok_or_else(|| format_err!("upload of {len} bytes is too large for a wasm theme"))?;

        let ptr = self.alloc.call(&mut self.store, cap)?;
        ensure!(ptr != 0, "theme's alloc({cap}) failed");
        ensure!(
            ptr % align_of::<u32>() as u32 == 0,
            "theme's alloc({cap}) returned a misaligned pointer at addr={ptr:#x}"
        );

        self.mailbox_buffer = Some((ptr, cap));
        Ok(ptr)
    }

    /// Runs the theme over `data`, which is a valid array of `kind`. Returns whether it changed.
    fn run(&mut self, kind: PrimitiveKind, data: &mut [u8]) -> Result<bool> {
        let len = u32::try_from(data.len())?;
        self.store.set_fuel(self.fuel_per_call)?;

        let ptr = self.reserve(len)?;
        self.memory.write(&mut self.store, ptr as usize, data)?;

        let theme_fn = match kind {
            PrimitiveKind::Quad => &self.theme_quad,
            PrimitiveKind::Shadow => &self.theme_shadow,
            PrimitiveKind::Underline => &self.theme_underline,
            PrimitiveKind::MonochromeSprite => &self.theme_monochrome_sprite,
            PrimitiveKind::SubpixelSprite => &self.theme_subpixel_sprite,
            PrimitiveKind::PolychromeSprite => &self.theme_polychrome_sprite,
        };

        let changed = theme_fn.call(&mut self.store, (ptr, len))? != 0;
        if changed {
            self.memory.read(&self.store, ptr as usize, data)?;
        }

        Ok(changed)
    }

    /// Applies the theme to `data`, which must be an array of `kind`.
    pub fn apply(&mut self, kind: PrimitiveKind, data: &mut [u8]) -> Option<bool> {
        if self.disabled {
            return None;
        }

        match self.run(kind, data) {
            Ok(changed) => Some(changed),
            Err(err) => {
                self.disabled = true;
                log::error!("theme failed to load on a {kind:?} upload: {err}");

                None
            }
        }
    }
}
