#[cfg(windows)]
mod windows;

use std::{borrow::Borrow, iter::once, num::NonZeroU8, sync::atomic::AtomicUsize};
#[cfg(feature = "llvm")]
use std::{borrow::Cow, collections::HashMap, num::NonZeroU64};

#[cfg(feature = "llvm")]
pub mod llvm;

#[cfg(feature = "llvm")]
pub mod symbpool;

#[cfg(windows)]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::*;

use crate::{
  Executable,
  relcar::{Relcar, Relocator},
  relocations::Relocation,
};

pub enum WriteFnResult {
  /// We have ran out of slab to allocate this
  OutOfSlab,
  /// The platform does not require [MemoryExecutableApi::seal] and it can be directly used
  ///
  /// Please note that this also flushes the iCache for good measure on some architectures (arm64, risv64, etc)
  Executable(*const Executable),
}

pub trait MemoryExecutableApi: Sized {
  const DEFAULT_SLAB_SIZE: usize = 16 * 1024 * 1024;

  /// Creates a new `16MB` slab to store machine code in
  ///
  /// ## Platform Specific implementation
  /// ### Windows, Linux
  /// We use RX, RW views strategy
  ///
  /// ### macOS
  /// We use a single view with macOS pthread_jit
  fn new_slab(multiple: Option<NonZeroU8>) -> Self;

  /// Writes a function into the data stream, returns `None` if the region is filled
  ///
  /// If the region is indeed filled, you're required create a new region, and seal the old region
  ///
  /// ## Safety
  /// This function relies on the accuracy of the [`capped_size`] field provided AND the accuracy
  /// of the total size with the final size [`capped_size`] field would provide.
  ///
  /// It is ONLY safe if [`capped_size`] <= size([`data`])
  unsafe fn write_fn_iterated<'a, T, E, R, B>(
    &mut self,
    alignment: usize,
    capped_size: usize,
    data: T,
    relocs: E,
    relcar: &Relcar<B>,
  ) -> WriteFnResult
  where
    T: Iterator<Item = &'a [u8]>,
    E: Iterator<Item = R>,
    R: Borrow<Relocation>,
    B: Relocator;

  /// Writes a function into the data stream, returns `None` if the region is filled
  ///
  /// If the region is indeed filled, you're required create a new region, and seal the old region
  ///
  /// This uses standard 16B alignment
  fn write_fn<B: Relocator>(
    &mut self,
    data: &[u8],
    relocs: &[Relocation],
    relcar: &Relcar<B>,
  ) -> WriteFnResult {
    unsafe { self.write_fn_iterated(16, data.len(), once(data), relocs.iter(), relcar) }
  }

  /// Makes that the FID can now be safely freed!
  /// We internally have a HashSet of the data and if all of them
  /// get freed, you are eligible to call `free`
  fn release(&self);

  /// Just like release but you provide the pointer
  /// to `self.stored`
  unsafe fn release_ptr(stored: &AtomicUsize);

  /// Deallocates the memory, file and all of the code stored
  ///
  /// This is safe because it checks if the `HashSet` is empty of not
  fn free(self) -> Result<(), Self>;

  /// Deallocates the memory, file and all of the code stored
  ///
  /// This is unsafe because you must `forget()` it after success
  unsafe fn try_free(&mut self) -> Result<(), ()>;

  /// Leak the data and forget HANDLES
  ///
  /// This is quite useful as it removes all the bookkeeping for Executable Code that
  /// won't be touched again!
  fn leak(self) -> ();
}

pub trait SizeCheck: MemoryExecutableApi {
  /// Does the MemoryExecutable have enough size
  fn under_size(&self, size: usize) -> Option<bool> {
    self.under_size_adv([SizeAlign { align: 16, size }].into_iter())
  }

  /// Does the MemoryExecutable have enough size
  fn under_size_adv<T>(&self, size_align: T) -> Option<bool>
  where
    T: Iterator<Item = SizeAlign>;

  /// Gets the base RX address
  fn base_address(&self) -> usize;
}

#[derive(Debug, Clone, Copy)]
pub struct SizeAlign {
  pub size: usize,
  pub align: usize,
}

impl SizeCheck for MemoryExecutable {
  fn under_size_adv<T>(&self, size_align: T) -> Option<bool>
  where
    T: Iterator<Item = SizeAlign>,
  {
    let base = (self.rxview as *const u8).addr().checked_add(self.cursor)?;

    let mut curr = base;
    let available = self.size.checked_sub(self.cursor)?;

    // Iterate & Check Overflow
    for item in size_align {
      let SizeAlign { size, align } = item;

      // Ensure alignment is a non-zero power of two if required by your layout
      if !align.is_power_of_two() {
        return None;
      }

      curr = curr.checked_next_multiple_of(align)?;
      curr = curr.checked_add(size)?;
    }

    let elasped = curr.checked_sub(base)?;
    Some(elasped <= available)
  }

  fn base_address(&self) -> usize {
    self.rxview.addr()
  }
}

pub trait MemorySizeInfo {
  fn size(&self) -> usize;
  fn cursor(&self) -> usize;

  fn next_cursor(&self, align: usize) -> Option<usize>;
}

impl MemorySizeInfo for MemoryExecutable {
  fn cursor(&self) -> usize {
    self.cursor
  }

  fn size(&self) -> usize {
    self.size
  }

  fn next_cursor(&self, alignment: usize) -> Option<usize> {
    let rx_base = (self.rxview as *const u8).addr();
    let base_addr = rx_base.checked_add(self.cursor)?;
    let cursor = base_addr
      .checked_next_multiple_of(alignment)?
      .checked_sub(rx_base)?;

    (cursor < self.size).then_some(cursor)
  }
}

#[cfg(feature = "llvm")]
pub trait LLVMDryRun: MemoryExecutableApi {
  /// Returns an approximated best-effort size (atmost size)
  /// by parsing the objectfile
  fn sizecalc(object: &[u8]) -> Option<NonZeroU64>;

  /// Returns an much more accurate best-effort size (atmost size)
  /// by parsing the objectfile
  fn sizecalc_jitlink(symbolpool: &symbpool::LLVMSymbolPool, object: &[u8]) -> Option<NonZeroU64>;
}

#[cfg(feature = "llvm")]
pub trait LLVMJITLink: MemoryExecutableApi {
  fn write_jitlink<T>(
    &mut self,
    symbolpool: &symbpool::LLVMSymbolPool,
    object: &[u8],
    resolver: T,
  ) -> Result<HashMap<Box<str>, *const Executable>, Cow<'static, [Cow<'static, str>]>>
  where
    T: FnMut(*const str) -> usize;
}

#[cfg(feature = "llvm")]
pub trait LLVMRTDyld: MemoryExecutableApi {
  fn write_rtdyld<T>(
    &mut self,
    object: &[u8],
    resolver: T,
  ) -> Result<HashMap<Box<str>, *const Executable>, ()>
  where
    T: FnMut(*const str) -> usize;
}
