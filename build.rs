#[rustfmt::skip]
fn main() {
  platform_check();

  #[cfg(feature = "llvm")]
  jitlink_llvm();
}

#[cfg(feature = "llvm")]
fn llvm_config(args: &[&str]) -> String {
  use std::{borrow::Cow, env::var, ffi::OsStr, process::Command};

  let program: Cow<'static, OsStr> = if let Ok(path) = var("LLVM_PATH") {
    use std::{env::consts::EXE_SUFFIX, path::PathBuf};

    let mut path = PathBuf::from(path);

    path.push("bin");
    path.push(format!("llvm-config{}", EXE_SUFFIX));

    Cow::Owned(path.into_os_string())
  } else {
    Cow::Borrowed(OsStr::new("llvm-config"))
  };

  let out = Command::new(program)
    .args(args)
    .output()
    .expect("llvm-config not found in PATH");

  String::from_utf8(out.stdout).expect("Invalid UTF8 was provided")
}

#[cfg(feature = "llvm")]
fn jitlink_llvm() {
  println!("cargo::rerun-if-changed=jitlinkc++");

  use cc::Build;

  let include_llvm = llvm_config(&["--includedir"]);

  Build::new()
    .cpp(true)
    .static_crt(true)
    .std("c++20")
    .file("./jitlinkc++/jitlink.cpp")
    .file("./jitlinkc++/rtdyld.cpp")
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
            target_arch = "loongarch64"
        ))
      )
    )
  )]
  compile_error!(
  "This package only works for:
 - Windows (x86_64, x86, aarch64)
 - Linux (arm, aarch64, x86_64, x86, loongarch64, riscv32, riscv64
 - MacOS (x86_64, aarch64)
  ");
}
