#[rustfmt::skip]
fn main() {
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
