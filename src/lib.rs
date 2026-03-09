pub mod advanced;
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
/// is guaranteed undefined behaviour that no CPU would like
/// and would result in memory access violation, or even worse,
/// crash with the OS.
pub struct Executable;

#[cfg(any(windows, target_os = "linux"))]
pub struct MemoryExecutable {
  map: Mmap,
}

#[cfg(any(windows, target_os = "linux"))]
impl MemoryExecutable {
  pub unsafe fn new_anon(
    machinecode: &[u8],
    reloc: &[Relocation],
  ) -> Result<Self, Box<dyn std::error::Error>> {
    let mut mmaput = MmapOptions::new().len(machinecode.len()).map_anon()?;
    mmaput.copy_from_slice(machinecode);

    Self::new(mmaput, reloc, machinecode.len())
  }

  #[inline(always)]
  fn new(
    mut mmaput: MmapMut,
    reloc: &[Relocation],
    len: usize,
  ) -> Result<Self, Box<dyn std::error::Error>> {
    for relocation in reloc {
      unsafe {
        relocate(mmaput.as_mut_ptr(), mmaput.len(), relocation);
      }
    }

    let map = mmaput.make_exec()?;

    flush_icache(map.as_ptr() as _, len);

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
unsafe fn relocate(mmap: *mut u8, len: usize, relocation: &Relocation) {
  let patch_site = unsafe { mmap.add(relocation.offset as _) };

  // NOTE: DANGER
  //
  // We do a direct bit reinterpretation to preserve the logic
  // wrapping_add ensures that stuff becomes negative
  let value = (relocation.symbol_addr as u64).wrapping_add(relocation.addend.cast_unsigned());

  match relocation.kind {
    RelocKind::Abs8 => unsafe {
      debug_assert_eq!(relocation.addend, 0);
      debug_assert!((relocation.offset as usize + 8) <= len);

      ptr::write_unaligned(patch_site as *mut u64, value);
    },
    #[cfg(target_pointer_width = "64")]
    RelocKind::Abs4 => unimplemented!("Unsupported platform"),
    #[cfg(not(target_pointer_width = "64"))]
    RelocKind::Abs4 => {
      debug_assert_eq!(relocation.addend, 0);
      debug_assert!((relocation.offset as usize + 4) <= len);

      ptr::write_unaligned(patch_site as *mut u32, value);
    }
    #[cfg(not(target_arch = "x86_64"))]
    RelocKind::X86CallPCRel4 | RelocKind::X86PCRel4 => unimplemented!("Unsupported platform"),
    #[cfg(target_arch = "x86_64")]
    RelocKind::X86CallPCRel4 | RelocKind::X86PCRel4 => {
      let target_addr = value as i64;
      let rip_after_instr = (patch_site as i64).wrapping_add(4);

      let displacement = target_addr.wrapping_sub(rip_after_instr);

      debug_assert!((relocation.offset as usize + 4) <= len);
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
    #[cfg(not(target_arch = "aarch64"))]
    RelocKind::Arm64Call => unimplemented!("Unsupported platform"),
    #[cfg(target_arch = "aarch64")]
    RelocKind::Arm64Call => {
      let target_addr = value as i64;
      let rip_after_instr = patch_site as i64;
      let displacement_bytes = target_addr.wrapping_sub(rip_after_instr);

      debug_assert_eq!(
        displacement_bytes % 4,
        0,
        "ARM64 branch target must be 4-byte aligned"
      );

      // OPTIMIZATION
      // Directly do division by `4`
      let displacement = displacement_bytes >> 2;

      debug_assert!((relocation.offset as usize + 4) <= len);

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
