#![feature(prelude_import)]
#[macro_use]
extern crate std;
#[prelude_import]
use std::prelude::rust_2024::*;
pub mod platform {
    use std::os::raw::c_void;
    mod win32 {
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
    }
    #[inline(always)]
    pub fn flush_icache(_base: *mut c_void, _size: usize) -> bool {
        true
    }
}
pub struct JitModule {}
