use std::{
  mem::forget,
  num::NonZeroU8,
  ptr::copy_nonoverlapping,
  sync::atomic::{AtomicUsize, Ordering, compiler_fence},
};

use windows::{
  Win32::{
    Foundation::{CloseHandle, GENERIC_EXECUTE, GENERIC_READ, GENERIC_WRITE, HANDLE},
    Storage::FileSystem::{
      CREATE_ALWAYS, CreateFileW, FILE_ATTRIBUTE_TEMPORARY, FILE_FLAG_DELETE_ON_CLOSE,
      FILE_SHARE_READ,
    },
    System::Memory::{
      CreateFileMappingW, FILE_MAP_EXECUTE, FILE_MAP_READ, FILE_MAP_WRITE,
      MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, PAGE_EXECUTE_READWRITE, UnmapViewOfFile,
    },
  },
  core::HSTRING,
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
  // File handle
  filehd: HANDLE,
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

unsafe fn create_file(path: impl AsRef<str>) -> HANDLE {
  unsafe {
    let path = path.as_ref();

    let hstr = HSTRING::from(path);

    let file = CreateFileW(
      &hstr,
      GENERIC_READ.0 | GENERIC_WRITE.0 | GENERIC_EXECUTE.0,
      FILE_SHARE_READ,
      None,
      CREATE_ALWAYS,
      FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_DELETE_ON_CLOSE,
      None,
    )
    .expect("Unable to create JIT File, Process crashing");

    file
  }
}

impl MemoryExecutableApi for MemoryExecutable {
  fn new_slab(path: impl AsRef<str>, multiple: Option<NonZeroU8>) -> Self {
    unsafe {
      let size =
        Self::DEFAULT_SLAB_SIZE.saturating_mul(multiple.map(|x| x.get()).unwrap_or(1) as _);

      let file = create_file(path);

      let mapping = CreateFileMappingW(
        file,
        None,
        PAGE_EXECUTE_READWRITE,
        (size >> 32) as u32,
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
        filehd: file,
        slab: mapping,
        rwview: rw_ptr as _,
        rxview: rx_ptr as _,
        size,
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

  fn release(&mut self) {
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
        CloseHandle(self.filehd).expect("Unable to close, delete file");
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
