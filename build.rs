#[rustfmt::skip]
fn main() {
  platform_check();

  #[cfg(feature = "llvm")]
  jitlink_llvm();
}

#[cfg(feature = "llvm")]
fn llvm_config(args: &[&str]) -> String {
  use std::{borrow::Cow, env::var, ffi::OsStr, process::Command};

  let vars = [
    var("SAJIT_LLVM_PATH"),
    var("LLVM_SYS_221_PREFIX"),
    var("LLVM_SYS_211_PREFIX"),
  ];

  let program: Cow<'static, OsStr> = if let Some(Ok(path)) = vars.into_iter().find(|v| v.is_ok()) {
    use std::{env::consts::EXE_SUFFIX, path::PathBuf};

    let mut path: PathBuf = PathBuf::from(path);

    path.push("bin");
    path.push(format!("llvm-config{}", EXE_SUFFIX));

    Cow::Owned(path.into_os_string())
  } else {
    Cow::Borrowed(OsStr::new("llvm-config"))
  };

  let out = Command::new(&program)
    .args(args)
    .output()
    .expect("llvm-config not found in PATH");

  if !out.status.success() {
    panic!(
      "Running `{}` failed. Kindly check if llvm-config correctly runs on your system.\n\nStdErr: {}",
      program.display(),
      String::from_utf8_lossy(&out.stderr)
    );
  }

  String::from_utf8(out.stdout).expect("Invalid UTF8 was provided")
}

#[cfg(feature = "llvm")]
fn jitlink_llvm() {
  use cc::Build;
  use std::{env, path::PathBuf};

  println!("cargo::rerun-if-changed=jitlinkc++");

  let bindings = bindgen::Builder::default()
    .header("jitlinkc++/jitlinkc++.h")
    .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
    .generate()
    .expect("Unable to generate bindings");

  let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
  bindings
    .write_to_file(out_path.join("bindings.rs"))
    .expect("Couldn't write bindings!");

  let include_llvm = llvm_config(&["--includedir"]);

  Build::new()
    .cpp(true)
    .std("c++20")
    .file("./jitlinkc++/jitlink.cpp")
    .file("./jitlinkc++/rtdyld.cpp")
    .file("./jitlinkc++/objcalc.cpp")
    .include("jitlinkc++")
    .include(include_llvm.trim())
    .compile("sajitlink");
}

#[rustfmt::skip]
fn platform_check() {
  #[cfg(
    not(
      any(
        // Windows Group
        all(target_os = "windows", any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")),
        
        // MacOS Group
        all(target_os = "macos", any(target_arch = "x86_64", target_arch = "aarch64")),
        
        // Linux Group
        all(target_os = "linux", any(
            target_arch = "x86_64", 
            target_arch = "x86", 
            target_arch = "aarch64", 
            target_arch = "arm", 
            target_arch = "riscv32", 
            target_arch = "riscv64", 
            target_arch = "loongarch64",
            target_arch = "powerpc64"
        ))
      )
    )
  )]
  compile_error!(
  "This package only works for:
 - Windows (x86_64, x86, aarch64)
 - Linux (arm, aarch64, x86_64, x86, loongarch64, riscv32, riscv64, powerpc64)
 - MacOS (x86_64, aarch64)
  ");
}
