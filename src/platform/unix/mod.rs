#[cfg(not(target_arch = "x86_64"))]
use core::ffi::c_void;

#[cfg(not(target_arch = "x86_64"))]
unsafe extern "C" {
  fn __clear_cache(start: *mut u8, end: *mut u8);
}

#[cfg(not(target_arch = "x86_64"))]
pub fn flush_icache(base: *mut c_void, size: usize) {
  let end = unsafe { base.add(size) };
  unsafe { __clear_cache(base as _, end as _) }
}
