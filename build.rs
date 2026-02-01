#[rustfmt::skip]
fn main() {
  #[cfg(
    any(
      not(
        any(
          target_arch = "x86_64",
          target_arch = "aarch64"
        ),
      ),
      not(
        any(target_os = "linux", windows)
      )
    )
  )]
  compile_error!("This package only works for x86_64 Windows and Linux runners");


  #[cfg(
    all(
      not(feature = "aarch64"),
      target_arch = "aarch64"
    )
  )]
  compile_error!("Arm64 support is currently experimental! Unless you know what you are doing, enable the `aarch64` feature");

  #[cfg(
    all(
      feature = "aarch64",
      target_arch = "aarch64"
    )
  )]
  println!("cargo:warning=`aarch64` JIT Loader is untested, it should work.");
}
