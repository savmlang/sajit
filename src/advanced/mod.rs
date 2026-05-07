#[cfg(windows)]
mod windows;

use std::num::NonZeroU8;
#[cfg(feature = "llvm")]
use std::{borrow::Cow, collections::HashMap};

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

use crate::{Executable, relocations::Relocation};

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

  /// Writes a function into the data stream, returns `None` if the 4KB region is filled
  ///
  /// If the region is indeed filled, you're required create a new region, and seal the old region
  fn write_fn(&mut self, data: &[u8], relocs: &[Relocation]) -> WriteFnResult;

  /// Makes that the FID can now be safely freed!
  /// We internally have a HashSet of the data and if all of them
  /// get freed, you are eligible to call `free`
  fn release(&self);

  /// Deallocates the memory, file and all of the code stored
  ///
  /// This is safe because it checks if the `HashSet` is empty of not
  fn free(self) -> Result<(), Self>;

  /// Leak the data and forget HANDLES
  ///
  /// This is quite useful as it removes all the bookkeeping for Executable Code that
  /// won't be touched again!
  fn leak(self) -> ();
}

#[cfg(feature = "llvm")]
pub trait LLVMJITLink: MemoryExecutableApi {
  fn write_llvm<T>(
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

#[cfg(feature = "llvm")]
pub trait LLVMBestLinking: LLVMRTDyld + LLVMJITLink {
  fn write_llvm<T>(
    &mut self,
    symbolpool: &symbpool::LLVMSymbolPool,
    object: &[u8],
    resolver: T,
  ) -> Result<HashMap<Box<str>, *const Executable>, Cow<'static, [Cow<'static, str>]>>
  where
    T: FnMut(*const str) -> usize;
}

#[cfg(feature = "llvm")]
impl<T: LLVMRTDyld + LLVMJITLink> LLVMBestLinking for T {
  fn write_llvm<E>(
    &mut self,
    symbolpool: &symbpool::LLVMSymbolPool,
    object: &[u8],
    resolver: E,
  ) -> Result<HashMap<Box<str>, *const Executable>, Cow<'static, [Cow<'static, str>]>>
  where
    E: FnMut(*const str) -> usize,
  {
    #[rustfmt::skip]
    const USE_RTDYLD: bool = cfg!(
      any(
        // Case A: Windows ARM64
        all(
          windows,
          target_arch = "aarch64"
        )
      )
    );

    if USE_RTDYLD {
      LLVMRTDyld::write_rtdyld(self, object, resolver).map_err(|_| {
        Cow::Borrowed(&[Cow::Borrowed("Unable to link using LLVMRTDyld")] as &[Cow<'static, str>])
      })
    } else {
      LLVMJITLink::write_llvm(self, symbolpool, object, resolver)
    }
  }
}
