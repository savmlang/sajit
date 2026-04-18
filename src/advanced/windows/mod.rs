use std::{
  mem::forget,
  num::NonZeroU8,
  ptr::copy_nonoverlapping,
  sync::atomic::{AtomicUsize, Ordering, compiler_fence},
};

use windows::Win32::{
  Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
  System::Memory::{
    CreateFileMappingW, FILE_MAP_EXECUTE, FILE_MAP_READ, FILE_MAP_WRITE,
    MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, PAGE_EXECUTE_READWRITE, UnmapViewOfFile,
  },
};

use crate::{
  Executable,
  advanced::{MemoryExecutableApi, WriteFnResult},
  relocate,
};

/// Unlike [crate::MemoryExecutable]
/// This is a File-Backed, Caching, Staging, Intelligent
/// Memory Mapper
///
/// This also stores an AtomicUsize to manage the lifecycle of itself
/// if the usize becomes `0` (i.e. all of the programs pointed by this have been cleared)
/// and the clear() method is called, it can & will drop itself
///
/// If you are unable to find a suitable file backing location, prefer using TEMP directory
#[repr(align(64))]
#[derive(Debug)]
pub struct MemoryExecutable {
  // Section object
  slab: HANDLE,

  // Views
  rxview: *const Executable,
  rwview: *mut u8,

  // Metadata
  size: usize,
  cursor: usize,
  stored: AtomicUsize,
}

impl MemoryExecutableApi for MemoryExecutable {
  fn new_slab(multiple: Option<NonZeroU8>) -> Self {
    unsafe {
      let size =
        Self::DEFAULT_SLAB_SIZE.saturating_mul(multiple.map(|x| x.get()).unwrap_or(1) as _);

      let mapping = CreateFileMappingW(
        INVALID_HANDLE_VALUE,
        None,
        PAGE_EXECUTE_READWRITE,
        (||{
          #[cfg(target_pointer_width = "64")]
          return (size >> 32) as u32;

          #[cfg(target_pointer_width = "32")]
          return 0;
        })(),
        size as u32,
        None,
      )
      .expect("Unable to create file mapping");

      let rw_ptr = MapViewOfFile(
        mapping,
        FILE_MAP_WRITE | FILE_MAP_READ,
        0,
        0,
        // Go upto file end
        0,
      )
      .Value;

      let rx_ptr = MapViewOfFile(
        mapping,
        FILE_MAP_EXECUTE | FILE_MAP_READ,
        0,
        0,
        // Go upto file end
        0,
      )
      .Value;

      // Now advance the cursor to the nearest 16B aligned block
      let cursor = rx_ptr.align_offset(16);

      Self {
        cursor,
        stored: AtomicUsize::new(0),
        slab: mapping,
        rwview: rw_ptr as _,
        rxview: rx_ptr as _,
        size: size as usize,
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
        use windows::Win32::System::{
          Diagnostics::Debug::FlushInstructionCache, Threading::GetCurrentProcess,
        };

        FlushInstructionCache(GetCurrentProcess(), Some(dst_rx as _), len);
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

  fn release(&self) {
    let _out = self.stored.fetch_sub(1, Ordering::Relaxed);
    debug_assert!(_out != 0);
  }

  fn free(self) -> Result<(), Self> {
    if self.stored.load(Ordering::Acquire) == 0 {
      unsafe {
        UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
          Value: self.rxview as _,
        })
        .expect("Could not unmap rx view");

        UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
          Value: self.rwview as _,
        })
        .expect("Could not unmap rx view");

        CloseHandle(self.slab).expect("Unable to close handle");
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
