use std::{ffi::c_void, ptr::NonNull};

use crate::relocations::llvmreloc::jitlink::{create_symbolpool, free_symbolpool};

pub struct LLVMSymbolPool {
  pub(crate) symbpool: NonNull<c_void>,
}

unsafe impl Send for LLVMSymbolPool {}
unsafe impl Sync for LLVMSymbolPool {}

impl LLVMSymbolPool {
  pub fn new() -> Self {
    unsafe {
      let symbpool = create_symbolpool();

      Self {
        symbpool: NonNull::new_unchecked(symbpool),
      }
    }
  }
}

impl Drop for LLVMSymbolPool {
  fn drop(&mut self) {
    unsafe {
      free_symbolpool(self.symbpool.as_ptr());
    }
  }
}
