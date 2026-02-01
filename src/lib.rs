pub mod platform;

use std::ptr;

pub use memmap2;
use memmap2::{Mmap, MmapMut, MmapOptions};

use crate::{
  platform::flush_icache,
  relocations::{RelocKind, Relocation},
};

pub mod relocations;

/// This is a wrapper structure
///
/// This is literally meant to annotate outputs
/// that are mounted with the RX flags.
///
/// This is in all of truth, just bytes, but it is
/// bytes in read+execute mode,
///
/// feel free to `transmute` it as anything that is
/// executable.
///
/// Casting a `*const Executable` as `*mut Executable`
/// is guaranteed undefined behaviour
///
/// No CPU would like this and would result in memory
/// access violation, or even worse, crash with the OS.
pub struct Executable;

pub struct MemoryExecutable {
  map: Mmap,
}

impl MemoryExecutable {
  pub fn new(machinecode: &[u8], reloc: &[Relocation]) -> Result<Self, Box<dyn std::error::Error>> {
    let mut mmaput = MmapOptions::new().len(machinecode.len()).map_anon()?;
    mmaput.copy_from_slice(machinecode);

    for relocation in reloc {
      unsafe {
        relocate(&mut mmaput, relocation);
      }
    }

    let map = mmaput.make_exec()?;

    flush_icache(map.as_ptr() as _, machinecode.len());

    Ok(Self { map })
  }

  /// Now, its your task to find out what it is
  ///
  /// It's fully safe from the standpoint of code,
  /// Its your responsibility to use it responsibly
  ///
  /// The function is not unsafe, but it acts as
  /// a reminder
  pub unsafe fn entry_ptr(&self) -> *const Executable {
    self.map.as_ptr() as _
  }
}

#[inline(always)]
unsafe fn relocate(mmap: &mut MmapMut, relocation: &Relocation) {
  let patch_site =
    unsafe { (mmap.as_mut_ptr() as *mut u8).add(relocation.offset as _) as *mut u64 };

  let value = (relocation.symbol_addr as i128 + relocation.addend as i128) as u64;

  match relocation.kind {
    RelocKind::Abs8 => unsafe {
      debug_assert_eq!(relocation.addend, 0);

      ptr::write_unaligned(patch_site, value);
    },
    // RelocKind::X86CallPCRel4 | RelocKind::X86PCRel4 => unsafe {
    //   // PC-relative: Target - (Current_Instruction_Pointer + 4)
    //   let delta = target_val - (patch_site as i64 + 4);
    //   ptr::write_unaligned(patch_site as *mut i32, delta as i32);
    // },
    _ => unimplemented!("SaJIT can only handle x86_64 Abs8 Instruction mapping"),
  }
}
