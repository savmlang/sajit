use std::os::raw::c_void;

#[cfg(windows)]
mod windows;

#[cfg(unix)]
mod unix;

#[rustfmt::skip]
#[cfg(
  any(
    target_arch = "x86_64",
    target_arch = "x86",
  )
)]
#[inline(always)]
pub fn flush_icache(_base: *mut c_void, _size: usize) -> bool {
  true
}

#[rustfmt::skip]
#[cfg(
  not(
    any(
      target_arch = "x86_64",
      target_arch = "x86",
    )
  )
)]
#[inline(always)]
pub fn flush_icache(base: *mut c_void, size: usize) -> bool {
  #[cfg(windows)]
  return windows::flush_icache(base, size);

  #[cfg(unix)]
  return unix::flush_icache(base, size);
}
