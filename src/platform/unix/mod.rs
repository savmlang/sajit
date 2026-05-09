#[cfg(not(target_arch = "x86_64"))]
use core::ffi::c_void;

#[cfg(all(not(target_arch = "x86_64"), target_os = "linux"))]
unsafe extern "C" {
  fn __clear_cache(start: *mut u8, end: *mut u8);
}

#[cfg(all(not(target_arch = "x86_64"), target_os = "macos"))]
unsafe extern "C" {
  fn sys_icache_invalidate(start: *mut c_void, len: usize);
}

#[cfg(all(not(target_arch = "x86_64"), target_os = "linux"))]
pub fn flush_icache(base: *mut c_void, size: usize) -> bool {
  let end = unsafe { base.add(size) };
  unsafe { __clear_cache(base as _, end as _) };

  true
}

#[cfg(all(not(target_arch = "x86_64"), target_os = "macos"))]
pub fn flush_icache(base: *mut c_void, size: usize) -> bool {
  unsafe { sys_icache_invalidate(base, size) };

  true
}
