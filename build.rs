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
  compile_error!("This package only works for x86_64 and aarch64 Windows and Linux runners");
}
