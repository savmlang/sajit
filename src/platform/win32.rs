#![allow(dead_code)]

use std::ffi::c_void;

#[link(name = "kernel32")]
unsafe extern "C" {
  unsafe fn GetCurrentProcess() -> *mut c_void;
  unsafe fn FlushInstructionCache(
    hProcess: *mut c_void,
    lpBaseAddress: *mut c_void,
    dwSize: usize,
  ) -> i32;
}

pub fn flush_icache(base: *mut c_void, size: usize) -> bool {
  unsafe { FlushInstructionCache(GetCurrentProcess(), base, size) != 0 }
}
