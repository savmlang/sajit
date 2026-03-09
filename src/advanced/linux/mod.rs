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
  relocate,
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
  rxview: *const Executable,
  rwview: *mut u8,

  // Metadata
  size: usize,
  cursor: usize,
  stored: AtomicUsize,
}

impl MemoryExecutableApi for MemoryExecutable {
  fn new_slab(_path: impl AsRef<str>, multiple: Option<NonZeroU8>) -> Self {
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

      let cursor = rx_ptr.align_offset(16);

      Self {
        fd,
        rxview: rx_ptr as _,
        rwview: rw_ptr as _,
        size,
        cursor, // Simplified: align this in your write_fn
        stored: AtomicUsize::new(0),
      }
    }
  }

  fn write_fn(
    &mut self,
    data: &[u8],
    relocs: &[crate::relocations::Relocation],
  ) -> super::WriteFnResult {
    let len = data.len();

    if self.cursor + len > self.size {
      return WriteFnResult::OutOfSlab;
    }

    unsafe {
      let start_offset = self.cursor;
      let dst_rw = self.rwview.byte_add(start_offset);
      let dst_rx = self.rxview.byte_add(start_offset);

      // Copy all the bytes
      copy_nonoverlapping(data.as_ptr(), dst_rw, len);

      // Relocate
      for relocation in relocs {
        relocate(dst_rw, len, relocation);
      }

      // Non X64 : Flush ICache
      #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
      {
        clear_cache::clear_cache(dst_rx as _, dst_rx.byte_add(len) as _);
      }

      compiler_fence(Ordering::Release);

      // 5. Advance cursor + Align for the NEXT function
      let next_raw = start_offset + len;
      // Find out padding
      let padding = (16 - (next_raw % 16)) % 16;
      self.cursor = next_raw + padding;

      self.stored.fetch_add(1, Ordering::Relaxed);

      WriteFnResult::Executable(dst_rx)
    }
  }

  fn release(&mut self) {
    let _old = self.stored.fetch_sub(1, Ordering::Relaxed);
    debug_assert!(_old != 0);
  }

  fn free(self) -> Result<(), Self> {
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
