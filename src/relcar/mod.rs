//! RELCAR
//!
//! **R**ust **E**fficient (Re)**L**ocator for Configurable Address Relocation
//!
//! This is SaJIT's own homegrown, extensible relocator

use crate::relocations::{RelocKind, Relocation};
use std::{marker::PhantomData, ptr};

pub static RELCAR_BASIC: Relcar<BasicRelocator> = Relcar {
  relcar_relocator: PhantomData,
};

pub struct Relcar<B: Relocator = BasicRelocator> {
  relcar_relocator: PhantomData<B>,
}

impl<B: Relocator> Default for Relcar<B> {
  fn default() -> Self {
    Self {
      relcar_relocator: PhantomData,
    }
  }
}

pub struct RelocInfo<'a> {
  pub mmap: *mut u8,
  pub len: usize,
  pub relocation: &'a Relocation,

  /// A calculated value
  ///
  /// exactly mmap+offset
  pub patch_site: *mut u8,
}

impl<B: Relocator> Relcar<B> {
  /// Alias to [`Self::default`]
  pub fn new() -> Self {
    Self::default()
  }

  #[inline(always)]
  /// Relocate an address using the [`Relcar`] interface
  pub fn relocate(&self, mmap: *mut u8, len: usize, relocation: &Relocation) {
    let patch_site = unsafe { mmap.add(relocation.offset as _) };

    #[cfg(target_arch = "aarch64")]
    let arm64callhwnd = |displacement_bytes: i64| {};

    let info = RelocInfo {
      mmap,
      len,
      relocation,
      patch_site,
    };

    match relocation.kind {
      RelocKind::Abs8 => {
        B::handle_abs8(info);
      }
      RelocKind::Abs4 => {
        B::handle_abs4(info);
      }
      RelocKind::X86CallPCRel4 => {
        B::handle_x86call_pc_rel4(info);
      }
      RelocKind::X86PCRel4 => {
        B::handle_x86_pc_rel4(info);
      }
      RelocKind::X86CallPCRelOrPCRelProvidedRelativeBytes => {
        B::handle_x86call_relbytes(info);
      }
      RelocKind::Arm64Call => {
        B::handle_arm64call(info);
      }
      RelocKind::Arm64CallProvidedRelativeBytes => {
        B::handle_arm64call_relbytes(info);
      }
      RelocKind::UserCustom { customdefined } => {
        B::handle_usercustom(info, customdefined);
      }
    }
  }
}

pub trait Relocator: Send + Sync {
  fn handle_abs8<'a>(info: RelocInfo<'a>) {
    let relocation = info.relocation;
    debug_assert_eq!(relocation.addend, 0);
    debug_assert!((relocation.offset as usize + 8) <= info.len);

    unsafe { ptr::write_unaligned(info.patch_site as *mut u64, info.relocation.symbol_addr) };
  }

  fn handle_abs4<'a>(info: RelocInfo<'a>) {
    debug_assert_eq!(info.relocation.addend, 0);
    debug_assert!((info.relocation.offset as usize + 4) <= info.len);

    unsafe {
      ptr::write_unaligned(
        info.patch_site as *mut u32,
        info.relocation.symbol_addr as u32,
      )
    };
  }

  fn handle_x86call_pc_rel4<'a>(info: RelocInfo<'a>) {
    x86rel(info);
  }

  fn handle_x86_pc_rel4<'a>(info: RelocInfo<'a>) {
    x86rel(info)
  }

  fn handle_x86call_relbytes(info: RelocInfo) {
    x86rel_i32(info.relocation.symbol_addr as u32 as i32, info.patch_site);
  }

  fn handle_arm64call(info: RelocInfo) {
    let target_addr = info.relocation.symbol_addr as i64;
    let rip_after_instr = info.patch_site as i64;
    let displacement_bytes = target_addr.wrapping_sub(rip_after_instr);

    arm64callhwnd(
      displacement_bytes,
      info.len,
      info.patch_site,
      info.relocation,
    );
  }

  fn handle_arm64call_relbytes(info: RelocInfo) {
    arm64callhwnd(
      info.relocation.symbol_addr.cast_signed(),
      info.len,
      info.patch_site,
      info.relocation,
    );
  }

  fn handle_usercustom(info: RelocInfo, userdefined: u16);
}

fn arm64callhwnd(displacement_bytes: i64, len: usize, patchsite: *mut u8, relocation: &Relocation) {
  debug_assert_eq!(
    displacement_bytes % 4,
    0,
    "ARM64 branch target must be 4-byte aligned"
  );

  let displacement = displacement_bytes / 4;

  debug_assert!((relocation.offset as usize + 4) <= len);

  #[cfg(debug_assertions)]
  if displacement > 0x1FFFFFF || displacement < -0x2000000 {
    panic!("Relocation truncated: Target is too far for ARM64 26-bit offset");
  }

  unsafe {
    let mut instruction = ptr::read_unaligned(patchsite as *const u32);

    instruction &= 0xFC000000;
    instruction |= (displacement as u32) & 0x03FFFFFF;

    ptr::write_unaligned(patchsite as *mut u32, instruction);
  }
}

fn x86rel<'a>(info: RelocInfo<'a>) {
  let target_addr = info.relocation.symbol_addr as i64;
  let rip_after_instr = (info.patch_site as i64).wrapping_add(4);

  let displacement = target_addr.wrapping_sub(rip_after_instr);

  debug_assert!((info.relocation.offset as usize + 4) <= info.len);
  #[cfg(debug_assertions)]
  if displacement > i32::MAX as _ || displacement < i32::MIN as _ {
    panic!("Relocation truncated to fit: Target is too far for 32-bit offset");
  }

  let displacement_32 = displacement as i32;
  x86rel_i32(displacement_32, info.patch_site);
}

fn x86rel_i32(displacement_32: i32, patchsite: *mut u8) {
  unsafe {
    // SAFETY:
    // x86_64 CPUs tolerate unaligned access, so this is okay even if the pointer is not 4-byte aligned.
    //
    // In practice, with Mmap and proper offsets, this pointer is aligned anyway.
    std::ptr::copy_nonoverlapping(
      &displacement_32 as *const i32 as *const u8,
      patchsite as *mut u8,
      4,
    );
  }
}

pub struct BasicRelocator;

impl Relocator for BasicRelocator {
  fn handle_usercustom(info: RelocInfo, _: u16) {
    panic!("Unknown relocation type : {:?}", info.relocation);
  }
}
