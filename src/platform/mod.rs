use std::os::raw::c_void;

#[cfg(windows)]
mod win32;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub fn flush_icache(_base: *mut c_void, _size: usize) -> bool {
  true
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub fn flush_icache(base: *mut c_void, size: usize) -> bool {
  #[cfg(windows)]
  return win32::flush_icache(base, size);

  #[cfg(target_os = "linux")]
  return linux::flush_icache(base, size);
}
