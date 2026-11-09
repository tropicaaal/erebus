# erebus

This is a proof-of-concept for a theme engine that can style Rust GPUI applications *after compilation* in a (somewhat) non-intrusive and stable way.

## Try it

<p align="center">
    <img src="./assets/demo.png" width="480" />
</p>

Make sure you have [Rust](https://rustup.rs/) installed. The example below injects into `zed` and should color everything black. You should be able to exchange `zed` for any GPUI app and get the same result.

```sh
cargo build --release
VK_LAYER_PATH=. VK_INSTANCE_LAYERS=VK_LAYER_erebus zed
```

## Why?

Discord has [announced](https://x.com/svishnevskiy/status/2118081920194203799) that they are switching away from using [Electron](https://www.electronjs.org/) in their desktop client in favor of [GPUI](https://gpui.rs). This is a great net improvement for end-users, but it does pretty much kill the client modding scene, where users would modify Discord's JavaScript and CSS to load custom themes and plugins.

Given the recent surge of interest in the Rust UI ecosystem, I expect that more apps are going to be moving away from Electron, which is going to make these kinds of modifications much harder in the long run. You can't just inject your own code into Rust binaries, since Rust doesn't have a stable ABI (and likely never will). This means that even if you do string together an elaborate client modification for one version of an app, any update could break everything at any time if someone decided to bump `rustc`.

*We need a solution that can modify Rust apps without touching the Rust ABI.*

## How?

The C ABI, of course! C is the lingua franca of programming languages and, [for better or for worse](https://faultlore.com/blah/c-isnt-a-language/), has had a stable ABI for decades. Some parts of GPUI need to use the C ABI for various reasons. One of those parts is the [`Scene` system](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/scene.rs).

In GPUI, a `Scene` is a complete plan for how to render a given frame of the application. It is built after the element tree has been evaluated right before data is uploaded to the GPU. And... that data uses the C ABI, because it has to when passing stuff to the GPU.

```rs
#[derive(Default, Debug, Copy, Clone)]
#[repr(C)] // <--- waow
#[expect(missing_docs)]
pub struct Quad {
    pub order: DrawOrder,
    pub border_style: BorderStyle,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub background: Background,
    pub border_color: Hsla,
    pub corner_radii: Corners<ScaledPixels>,
    pub border_widths: Edges<ScaledPixels>,
}
```

The rust crate in this repo is a [Vulkan Layer](https://docs.vulkan.org/guide/latest/layers.html), which is a small shared object that hooks functions exposed by the Vulkan API. It keeps track of all allocations that GPUI makes through Vulkan and intercepts `vkCmdCopyBuffer` and `vkCmdCopyBuffer2` to access these drawing primitives and modify them right before they reach the GPU.

This effectively makes a "theme" a function that runs over every drawing primitive, makes the necessary modifications, then returns. Pretty cool!

```rs
// example theme that turns everything black
fn theme(quad: &mut Quad) {
    quad.background = Background::solid(Hsla::from_rgba(0, 0, 0, 255));
}
```

## Caveats

GPUI uses a different rendering backend on each platform and will only ever use Vulkan on linux via `wgpu` if the system supports it. Unfortunately, this means that this method **only works on linux devices supporting Vulkan**. This doesn't mean that something similar can't be adapted for other platforms, either by also hooking lowlevel graphics APIs, or by locating `Scene` in memory through other means (DWARF debuginfo, `std::panic::Location` instances in rodata, etc...) This *can* be generalized to other platforms, it just needs a different injection method for each.

Additionally, because themes operate on drawing primitives after the element tree has been evaluated, we don't get any semantic meaning to what we're themeing. This is distinctly different from something like CSS. There are no selectors, you're just modifying a list of paint operations. You can tell if a box being rendered is blue or red, but you can't tell if it's a button or a checkbox. This is a fundamental limitation of this system (and GPUI as a whole; it has no concept of a selector). I don't have a solution for this, and I don't think there is one without touching the Rust ABI.
