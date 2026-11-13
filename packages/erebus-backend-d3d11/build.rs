// //! I HATE WINAPI!
// //!
// //! erebus-d3d11 uses DLL hijacking to get the app to load our own d3d11.dll
// //! instead of the one in C:/Windows/System32/d3d11.dll. The problem is that
// //! we are now expected to provide all of the procedures in the original DLL
// //! through our replaced copy.
// //!
// //! If we just used the output DLL as is, GPUI would fail to start because
// //! it relies on more than just the functions we patch... So, we need to read
// //! an d3d11.dll and forward every export *other* than the functions we hook.
// //!
// //! Also, annoyingly, we can't just forward to the d3d11.dll in System32, since
// //! .def files don't support absolute paths. If we just pointed the forwarded
// //! functions to LIBRARY d3d11, then we'd be pointing at ourselves because again,
// //! we're hijacking the dll. Instead we expect a copy of the original dll to be
// //! present in the search path at d3d11_orig.dll. Yeesh, I hate windows.

// use std::{path::PathBuf, process::Command};

// use goblin::Object;

// const HOOKED_FUNCTIONS: &[&str] = &["D3D11CreateDevice", "D3D11CreateDeviceAndSwapChain"];

// fn main() {
//     let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

//     // find the original d3d11.dll in System32
//     let system_d3d11 =
//         PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
//             .join(r"System32\d3d11.dll");

//     println!("cargo:rerun-if-changed={}", system_d3d11.display());

//     // parse PE object
//     let bytes = std::fs::read(&system_d3d11).expect("failed to read system d3d11.dll");
//     let pe = match Object::parse(&bytes).expect("failed to parse d3d11.dll") {
//         Object::PE(pe) => pe,
//         _ => panic!("d3d11.dll is not a PE file"),
//     };

//     // collect exports that should be forwarded (literally everything except
//     // what we specify in HOOKED_FUNCTIONS)
//     let forwarders: Vec<_> = pe
//         .exports
//         .iter()
//         .filter_map(|export| export.name)
//         .filter(|name| !HOOKED_FUNCTIONS.contains(name))
//         .collect();

//     // Next we're gonna make two different definition files. One for erebus and one for
//     // d3d11_orig.dll. The one for erebus (proxy_def)
//     //
//     // http://learn.microsoft.com/en-us/cpp/build/exporting-from-a-dll-using-def-files
//     //
//     // The first one points functions in our hijack library towards ones in d3d11_orig.dll,
//     // and passes that to the linker to tell it what to export. The second one tells LIB.exe
//     // what symbols are in d3d11_orig.dll so it can generate us an import file.

//     let erebus_def_path = out_dir.join("d3d11_forwarders.def");
//     let orig_def_path = out_dir.join("d3d11_orig.def");

//     // generate the export definition for erebus. that looks like this:
//     //
//     // EXPORTS
//     // ForwardedFunction1=d3d11_orig.ForwardedFunction1
//     // ForwardedFunction2=d3d11_orig.ForwardedFunction2
//     // ...
//     let mut erebus_def = String::from("EXPORTS\n");
//     for name in &forwarders {
//         erebus_def.push_str(&format!("{name}=d3d11_orig.{name}\n"));
//     }

//     std::fs::write(&erebus_def_path, erebus_def).unwrap();
//     println!("cargo:rustc-link-arg=/DEF:{}", erebus_def_path.display());

//     // generate the import definition for d3d11_orig.dll. that looks like this:
//     //
//     // LIBRARY d3d11_orig
//     // EXPORTS
//     // ForwardedFunction1
//     // ForwardedFunction2
//     // ...
//     let mut orig_def = String::from("LIBRARY d3d11_orig\nEXPORTS\n");
//     for name in &forwarders {
//         orig_def.push_str(&format!("{name}\n"));
//     }
//     std::fs::write(&orig_def_path, orig_def).unwrap();

//     let lib_path = out_dir.join("d3d11_orig.lib");
//     let output = Command::new("lib.exe")
//         .arg(format!("/DEF:{}", orig_def_path.display()))
//         .arg("/MACHINE:X64") // TODO arm64
//         .arg(format!("/OUT:{}", lib_path.display()))
//         .output()
//         .expect("failed to run lib.exe, make sure your MSVC toolchain is in PATH");

//     if !output.status.success() {
//         panic!(
//             "lib.exe failed:\n{}",
//             String::from_utf8_lossy(&output.stderr)
//         );
//     }

//     println!("cargo:rustc-link-arg={}", lib_path.display());
// }

fn main() {}