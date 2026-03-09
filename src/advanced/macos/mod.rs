use std::{
  mem::forget,
  ptr::copy_nonoverlapping,
  ptr::null_mut,
  sync::atomic::{AtomicUsize, Ordering, compiler_fence},
};

use libc::{
  MAP_ANON, MAP_JIT, MAP_PRIVATE, MFD_CLOEXEC, PROT_EXEC, PROT_READ, PROT_WRITE, mmap, munmap,
};

use crate::{
  Executable,
  advanced::{MemoryExecutableApi, WriteFnResult},
  relocate,
};

#[link(name = "pthread")]
unsafe extern "C" {
  fn pthread_jit_write_protect_np(enabled: i32);
}

/// Unlike [crate::MemoryExecutable]
/// This is a memfd File-Backed, Caching, Staging, Intelligent
/// Memory Mapper
///
/// This also stores an AtomicUsize to manage the lifecycle of itself
/// if the usize becomes `0` (i.e. all of the programs pointed by this have been cleared)
/// and the clear() method is called, it can & will drop itself
#[repr(align(64))]
#[derive(Debug)]
pub struct MemoryExecutable {
  // View
  rview: *mut u8,

  // Metadata
  size: usize,
  cursor: usize,
  stored: AtomicUsize,
}

impl MemoryExecutableApi for MemoryExecutable {
  type FID = u64;

  fn new_slab(_path: impl AsRef<str>) -> Self {
    unsafe {
      let rview = mmap(
        null_mut(),
        Self::DEFAULT_SLAB_SIZE as _,
        PROT_READ | PROT_WRITE | PROT_EXEC,
        MAP_ANON | MAP_PRIVATE | MAP_JIT,
        -1,
        0,
      );

      let cursor = rview.align_offset(16);

      Self {
        rview,
        size: Self::DEFAULT_SLAB_SIZE,
        cursor,
        stored: AtomicUsize::new(0),
      }
    }
  }

  fn write_fn(
    &mut self,
    _fid: Self::FID,
    data: &[u8],
    relocs: &[crate::relocations::Relocation],
  ) -> super::WriteFnResult {
    let len = data.len();

    if self.cursor + len > self.size {
      return WriteFnResult::OutOfSlab;
    }

    unsafe {
      let start_offset = self.cursor;
      let dst_rw = self.rview.byte_add(start_offset);

      pthread_jit_write_protect_np(0);
      // Copy all the bytes
      copy_nonoverlapping(data.as_ptr(), dst_rw, len);

      // Relocate
      for relocation in relocs {
        relocate(dst_rw, len, relocation);
      }
      pthread_jit_write_protect_np(1);

      // Non X64 : Flush ICache
      #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
      {
        clear_cache::clear_cache(dst_rw as _, dst_rw.byte_add(len) as _);
      }

      compiler_fence(Ordering::Release);

      // 5. Advance cursor + Align for the NEXT function
      let next_raw = start_offset + len;
      // Find out padding
      let padding = (16 - (next_raw % 16)) % 16;
      self.cursor = next_raw + padding;

      self.stored.fetch_add(1, Ordering::Relaxed);

      WriteFnResult::Executable(dst_rw)
    }
  }

  fn seal(&mut self) -> Option<Box<[Self::FID]>> {
    None
  }

  fn release(&mut self, _fid: Self::FID) {
    let _old = self.stored.fetch_sub(1, Ordering::Relaxed);
    debug_assert!(_old != 0);
  }

  fn free(self) -> Result<(), Self> {
    if self.stored.load(Ordering::Acquire) == 0 {
      unsafe {
        let output = munmap(self.rview as _, self.size);

        if output != 0 {
          panic!("Unable to correct free dependencies");
        }
      }

      forget(self);

      return Ok(());
    }

    Err(self)
  }

  fn leak(self) -> () {
    forget(self);
  }
}

impl Drop for MemoryExecutable {
  fn drop(&mut self) {
    panic!("MemoryExecutable has been dropped, please note that this is undefined behaviour!")
  }
}
