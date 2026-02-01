use core::ffi::c_void;

#[cfg(not(target_arch = "x86_64"))]
use clear_cache::clear_cache;

#[cfg(target_arch = "x86_64")]
pub fn flush_icache(_base: *mut c_void, _size: usize) -> bool {
  true
}

#[cfg(not(target_arch = "x86_64"))]
pub fn flush_icache(base: *mut c_void, size: usize) -> bool {
  let end = unsafe { base.add(size) };
  unsafe { clear_cache(base, end) }
}
