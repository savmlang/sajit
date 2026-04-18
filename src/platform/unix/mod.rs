#[cfg(not(target_arch = "x86_64"))]
use core::ffi::c_void;

#[cfg(
  all(
    not(target_arch = "x86_64"),
    target_os = "linux"
  )
)]
unsafe extern "C" {
  fn __clear_cache(start: *mut u8, end: *mut u8);
}

#[cfg(
  all(
    not(target_arch = "x86_64"),
    target_os = "linux"
  )
)]
pub fn flush_icache(base: *mut c_void, size: usize) {
  let end = unsafe { base.add(size) };
  unsafe { __clear_cache(base as _, end as _) }
}

#[cfg(
  all(
    not(target_arch = "x86_64"),
    target_os = "macos"
  )
)]
pub fn flush_icache(base: *mut c_void, size: usize) -> bool {
  let end = unsafe { base.add(size) };
  unsafe { clear_cache::clear_cache(base, end) }
}
