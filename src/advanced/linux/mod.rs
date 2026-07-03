use std::{
  mem::forget,
  num::NonZeroU8,
  ptr::copy_nonoverlapping,
  ptr::null_mut,
  sync::atomic::{AtomicUsize, Ordering, compiler_fence},
};

use libc::{
  MAP_SHARED, MFD_CLOEXEC, PROT_EXEC, PROT_READ, PROT_WRITE, close, ftruncate, memfd_create, mmap,
  munmap,
};

use crate::{
  Executable,
  advanced::{MemoryExecutableApi, WriteFnResult},
  relcar::Relcar,
};

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
  fd: i32,
  // Views
  pub(crate) rxview: *const Executable,
  pub(crate) rwview: *mut u8,

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

      let fd = memfd_create(b"sajit\0".as_ptr() as _, MFD_CLOEXEC);
      if fd == -1 {
        panic!("Failed to create memfd");
      }

      ftruncate(fd, size as _);

      let rw_ptr = mmap(
        null_mut(),
        size as _,
        PROT_READ | PROT_WRITE,
        MAP_SHARED,
        fd,
        0,
      );

      let rx_ptr = mmap(
        null_mut(),
        size as _,
        PROT_READ | PROT_EXEC,
        MAP_SHARED,
        fd,
        0,
      );

      Self {
        fd,
        rxview: rx_ptr as _,
        rwview: rw_ptr as _,
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
      let dst_rw = self.rwview.byte_add(start_offset);
      let dst_rx = self.rxview.byte_add(start_offset);

      // Copy all the bytes
      copy_nonoverlapping(data.as_ptr(), dst_rw, len);

      // Relocate
      for relocation in relocs {
        relcar.relocate(dst_rw, len, relocation);
      }

      // Non X64 : Flush ICache
      // X64 : NOOP
      crate::platform::flush_icache(dst_rx as _, len);

      compiler_fence(Ordering::Release);

      // 5. Advance cursor
      let next_raw = start_offset + len;

      // Let the other section decide alignment
      self.cursor = next_raw;

      self.stored.fetch_add(1, Ordering::Relaxed);

      WriteFnResult::Executable(dst_rx)
    }
  }

  fn release(&self) {
    unsafe { Self::release_ptr(&self.stored) }
  }

  unsafe fn release_ptr(stored: &AtomicUsize) {
    let _old = stored.fetch_sub(1, Ordering::Relaxed);
    debug_assert!(_old != 0);
  }

  fn free(mut self) -> Result<(), Self> {
    if let Ok(()) = unsafe { self.try_free() } {
      forget(self);
      return Ok(());
    }

    Err(self)
  }

  unsafe fn try_free(&mut self) -> Result<(), ()> {
    if self.stored.load(Ordering::Acquire) == 0 {
      unsafe {
        let output = [
          munmap(self.rwview as _, self.size).cast_unsigned(),
          munmap(self.rxview as _, self.size).cast_unsigned(),
          close(self.fd).cast_unsigned(),
        ]
        .into_iter()
        .sum::<u32>();

        if output != 0 {
          panic!("Unable to correct free dependencies");
        }
      }

      return Ok(());
    }

    Err(())
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
