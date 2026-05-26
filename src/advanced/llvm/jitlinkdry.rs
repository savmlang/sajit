use std::num::NonZeroU64;

use crate::{
  LLVMDryRun, MemoryExecutable,
  relocations::llvmreloc::{calc, calc_jitlink},
  symbpool::LLVMSymbolPool,
};

impl LLVMDryRun for MemoryExecutable {
  fn sizecalc(object: &[u8]) -> Option<NonZeroU64> {
    let len = unsafe { calc(object.as_ptr() as _, object.len()) };
    NonZeroU64::new(len)
  }

  fn sizecalc_jitlink(symbolpool: &LLVMSymbolPool, object: &[u8]) -> Option<NonZeroU64> {
    let len = unsafe {
      calc_jitlink(
        symbolpool.symbpool.as_ptr(),
        object.as_ptr() as _,
        object.len(),
      )
    };
    NonZeroU64::new(len)
  }

  fn under_size(&self, size: usize) -> Option<bool> {
    Some(
      self
        .cursor
        // Since we've calculated size from 1B alignment perspective, we can directly add it
        .checked_add(size)?
        <= self.size,
    )
  }
}
