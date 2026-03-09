#[cfg(windows)]
mod windows;

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
  /// This region is done for, use `seal` to move to the next one
  OutOfRegion,
  /// The pointer has been written in the region, but it is awaiting [MemoryExecutableApi::seal] to become executable
  ProvisionalPtr(*const Executable),
  /// The platform does not require [MemoryExecutableApi::seal] and it can be directly used
  ///
  /// Please note that this also flushes the iCache for good measure on some architectures (arm64, risv64, etc)
  Executable(*const Executable),
}

pub trait MemoryExecutableApi: Sized {
  type FID;

  const DEFAULT_SLAB_SIZE: usize = 16 * 1024 * 1024;

  /// Creates a new `16MB` slab to store machine code in
  ///
  /// There are 4KB regions inside this 16MB slab
  /// The total is `4000` regions
  ///
  /// This math is more/less for UNIX like systems only
  fn new_slab(path: impl AsRef<str>) -> Self;

  // /// Creates a new LARGE slab to store machine code in
  // ///
  // /// This is guaranteed to be atleast `32MB`
  // ///
  // /// ## Platform Notes
  // /// ### Windows
  // /// On windows, the size and the number of regions is not defined or fixed, hence it is better
  // /// to depend on the apis to detect when the region ends!
  // ///
  // /// We try to allocate as much as fits within 64MB
  // fn new_slab_large(path: impl AsRef<str>) -> Self;

  /// Writes a function into the data stream, returns `None` if the 4KB region is filled
  ///
  /// If the region is indeed filled, you're required create a new region, and seal the old region
  ///
  /// The `fid` is what will be returned on [MemoryExecutableApi::seal] operation
  fn write_fn(&mut self, fid: Self::FID, data: &[u8], relocs: &[Relocation]) -> WriteFnResult;

  /// Seals the region and returns all of the `FID`s that are now executables
  ///
  /// On some platforms, seal is not required and [MemoryExecutableApi::write_fn] would return [WriteFnResult::Executable] to reflect the same
  fn seal(&mut self) -> Option<Box<[Self::FID]>>;

  /// Makes that the FID can now be safely freed!
  /// We internally have a HashSet of the data and if all of them
  /// get freed, you are eligible to call `free`
  fn release(&mut self, fid: Self::FID);

  /// Deallocates the memory, file and all of the code stored
  ///
  /// This is safe because it checks if the `HashSet` is empty of not
  fn free(self) -> Result<(), Self>;

  /// TO DO: Leak the data and forget HANDLES
  ///
  /// This is quite useful as it removes all the bookkeeping for Executable Code that
  /// won't be touched again!
  fn leak(self) -> ();
}
