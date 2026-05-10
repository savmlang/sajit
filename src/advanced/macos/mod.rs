use std::{
  mem::forget,
  num::NonZeroU8,
  ptr::copy_nonoverlapping,
  ptr::null_mut,
  sync::atomic::{AtomicUsize, Ordering, compiler_fence},
};

use libc::{MAP_ANON, MAP_JIT, MAP_PRIVATE, PROT_EXEC, PROT_READ, PROT_WRITE, mmap, munmap};

#[cfg(feature = "llvm")]
use crate::Executable;

use crate::{
  advanced::{MemoryExecutableApi, WriteFnResult},
  relcar::Relcar,
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
  #[cfg(feature = "llvm")]
  pub(crate) rwview: *mut u8,
  #[cfg(feature = "llvm")]
  pub(crate) rxview: *const Executable,

  // Metadata
  pub(crate) size: usize,
  pub(crate) cursor: usize,
  pub stored: AtomicUsize,
}

impl MemoryExecutableApi for MemoryExecutable {
  fn new_slab(multiple: Option<NonZeroU8>) -> Self {
    unsafe {
      let size =
        Self::DEFAULT_SLAB_SIZE.saturating_mul(multiple.map(|x| x.get()).unwrap_or(1) as _);

      let rview = mmap(
        null_mut(),
        size as _,
        PROT_READ | PROT_WRITE | PROT_EXEC,
        MAP_ANON | MAP_PRIVATE | MAP_JIT,
        -1,
        0,
      ) as *mut u8;

      Self {
        rview,
        #[cfg(feature = "llvm")]
        rwview: rview,
        #[cfg(feature = "llvm")]
        rxview: rview as _,
        size,
        cursor: 0,
        stored: AtomicUsize::new(0),
      }
    }
  }

  fn write_fn(
    &mut self,
    data: &[u8],
    relocs: &[crate::relocations::Relocation],
    relcar: &Relcar,
  ) -> super::WriteFnResult {
    let len = data.len();

    let start_offset = self.cursor.next_multiple_of(16);

    if start_offset + len > self.size {
      return WriteFnResult::OutOfSlab;
    }

    unsafe {
      let dst_rwx = self.rview.byte_add(start_offset);

      pthread_jit_write_protect_np(0);
      // Copy all the bytes
      copy_nonoverlapping(data.as_ptr(), dst_rwx, len);

      // Relocate
      for relocation in relocs {
        relcar.relocate(dst_rwx, len, relocation);
      }
      pthread_jit_write_protect_np(1);

      // Non X64 : Flush ICache
      // X64 : NOOP
      crate::platform::flush_icache(dst_rwx as _, len);

      compiler_fence(Ordering::Release);

      // 5. Advance cursor
      let next_raw = start_offset + len;

      // Let the other section decide alignment
      self.cursor = next_raw;

      self.stored.fetch_add(1, Ordering::Relaxed);

      WriteFnResult::Executable(dst_rwx as _)
    }
  }

  fn release(&self) {
    unsafe { Self::release_ptr(&self.stored) }
  }

  unsafe fn release_ptr(stored: &AtomicUsize) {
    let _old = stored.fetch_sub(1, Ordering::Relaxed);
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
