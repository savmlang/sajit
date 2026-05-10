use std::{ffi::c_void, slice::from_raw_parts};

use crate::{
  LLVMDryRun, MemoryExecutable,
  relocations::llvmreloc::{SizeAlignInfoCS, rtdyld_dryrun},
};

impl LLVMDryRun for MemoryExecutable {
  fn size_jitlink(&mut self, object: &[u8]) -> Option<usize> {
    let mut size = None;

    unsafe {
      rtdyld_dryrun(
        &mut size as *mut _ as _,
        Some(sizealign),
        object.as_ptr() as _,
        object.len(),
      )
    };

    size
  }

  fn size_rtdylb(&mut self, object: &[u8]) -> Option<usize> {
    let mut size = None;

    unsafe {
      rtdyld_dryrun(
        &mut size as *mut _ as _,
        Some(sizealign),
        object.as_ptr() as _,
        object.len(),
      );
    };

    size
  }

  fn under_size(&mut self, size: usize) -> Option<bool> {
    Some(
      self
        .cursor
        // Since we've calculated size from 1B alignment perspective, we can directly add it
        .checked_add(size)?
        <= self.size,
    )
  }
}

extern "C" fn sizealign(state: *mut c_void, sizealigninfo: SizeAlignInfoCS) {
  unsafe {
    let src = from_raw_parts(sizealigninfo.ptr, sizealigninfo.size);

    let val = (|| {
      const DANGLING_DEFAULT: usize = 1;

      // Size from a reference 1B aligned DANGLING pointer
      let mut fakeptr: usize = DANGLING_DEFAULT;

      for sizeitem in src {
        fakeptr = fakeptr.checked_next_multiple_of(sizeitem.align.max(1) as _)?;

        fakeptr = fakeptr.checked_add(sizeitem.size)?;
      }

      (fakeptr.checked_sub(DANGLING_DEFAULT)?).checked_next_multiple_of(DANGLING_DEFAULT)
    })();

    *(state as *mut Option<usize>) = val;
  }
}
