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
  pub unsafe fn new(
    machinecode: &[u8],
    reloc: &[Relocation],
  ) -> Result<Self, Box<dyn std::error::Error>> {
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
  let patch_site = unsafe { (mmap.as_mut_ptr() as *mut u8).add(relocation.offset as _) };

  let value = (relocation.symbol_addr as i128 + relocation.addend as i128) as u64;

  match relocation.kind {
    RelocKind::Abs8 => unsafe {
      debug_assert_eq!(relocation.addend, 0);
      debug_assert!((relocation.offset as usize + 8) <= mmap.len());

      ptr::write_unaligned(patch_site as *mut u64, value);
    },
    #[cfg(not(target_arch="x86_64"))]
    RelocKind::X86CallPCRel4 | RelocKind::X86PCRel4 => unimplemented!("Unsupported platform"),
    #[cfg(target_arch="x86_64")]
    RelocKind::X86CallPCRel4 | RelocKind::X86PCRel4 => {
      let displacement = (value as i128) - (patch_site as i128 + 4);

      debug_assert!((relocation.offset as usize + 4) <= mmap.len());
      #[cfg(debug_assertions)]
      if displacement > i32::MAX as _ || displacement < i32::MIN as _ {
        panic!("Relocation truncated to fit: Target is too far for 32-bit offset");
      }

      let displacement_32 = displacement as i32;
      unsafe {
        // SAFETY:
        // x86_64 CPUs tolerate unaligned access, so this is okay even if the pointer is not 4-byte aligned.
        //
        // In practice, with Mmap and proper offsets, this pointer is aligned anyway.
        std::ptr::copy_nonoverlapping(
          &displacement_32 as *const i32 as *const u8,
          patch_site as *mut u8,
          4,
        );
      }
    }
    #[cfg(not(target_arch="aarch64"))]
    RelocKind::Arm64Call => unimplemented!("Unsupported platform"),
    #[cfg(target_arch="aarch64")]
    RelocKind::Arm64Call => {
      let displacement_bytes = (value as i128) - (patch_site as i128);
      let displacement = displacement_bytes / 4;

      debug_assert!((relocation.offset as usize + 4) <= mmap.len());

      #[cfg(debug_assertions)]
      if displacement > 0x1FFFFFF || displacement < -0x2000000 {
        panic!("Relocation truncated: Target is too far for ARM64 26-bit offset");
      }

      unsafe {
        let mut instruction = ptr::read_unaligned(patch_site as *const u32);

        instruction &= 0xFC000000;
        instruction |= (displacement as u32) & 0x03FFFFFF;

        ptr::write_unaligned(patch_site as *mut u32, instruction);
      }
    }
  }
}
