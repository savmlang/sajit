use std::ptr;

use object::pe::{
  IMAGE_REL_ARM64_ABSOLUTE, IMAGE_REL_ARM64_ADDR32, IMAGE_REL_ARM64_ADDR32NB,
  IMAGE_REL_ARM64_ADDR64, IMAGE_REL_ARM64_BRANCH14, IMAGE_REL_ARM64_BRANCH19,
  IMAGE_REL_ARM64_BRANCH26, IMAGE_REL_ARM64_PAGEBASE_REL21, IMAGE_REL_ARM64_PAGEOFFSET_12A,
  IMAGE_REL_ARM64_PAGEOFFSET_12L, IMAGE_REL_ARM64_REL21, IMAGE_REL_ARM64_REL32,
  IMAGE_REL_ARM64_SECTION,
};

use crate::coffr::{
  CoFFRError,
  arch::{LinkSection, ResolvedRelocation},
};

pub fn relocate<A, B>(
  imagebase: u64,
  text: LinkSection<A>,
  rdata: Option<LinkSection<B>>,
) -> Result<(), CoFFRError>
where
  A: Iterator<Item = Result<ResolvedRelocation, CoFFRError>>,
  B: Iterator<Item = Result<ResolvedRelocation, CoFFRError>>,
{
  unsafe {
    link(imagebase, text)?;

    if let Some(rdata) = rdata {
      link(imagebase, rdata)?;
    }

    Ok(())
  }
}

unsafe fn link<A>(base: u64, section: LinkSection<A>) -> Result<(), CoFFRError>
where
  A: Iterator<Item = Result<ResolvedRelocation, CoFFRError>>,
{
  let view = section.view;

  let b = base as i64;
  for relocation in section.reloc {
    let relocation = relocation?;

    let s_idx = relocation.sectidx;
    let s = relocation.symbol as i64;
    let p = view.rx_ptr.addr() as i64 + relocation.position_offset as i64;

    let p_rw = unsafe { view.rw_ptr.add(relocation.position_offset as _) };

    unsafe {
      match relocation.typ {
        IMAGE_REL_ARM64_ABSOLUTE => continue,

        IMAGE_REL_ARM64_ADDR64 => {
          let a = ptr::read_unaligned(p_rw as *mut u64);

          ptr::write_unaligned(p_rw as *mut u64, s.cast_unsigned() + a);
        }

        IMAGE_REL_ARM64_SECTION => {
          ptr::write_unaligned(p_rw as *mut u16, s_idx);
        }

        IMAGE_REL_ARM64_ADDR32 => {
          let a = ptr::read_unaligned(p_rw as *mut u32) as i64;
          let target = s + a;

          within_bits::<false, 32>(target)?;
          ptr::write_unaligned(p_rw as *mut u32, target as _);
        }

        IMAGE_REL_ARM64_ADDR32NB => {
          let a = ptr::read_unaligned(p_rw as *mut u32) as i64;
          let target = s + a - b;

          within_bits::<false, 32>(target)?;
          ptr::write_unaligned(p_rw as *mut u32, target as _);
        }

        // 26-bit offset
        // signext
        // <<2
        IMAGE_REL_ARM64_BRANCH26 => {
          const BITS: u8 = 26;
          const SIGNED: bool = true;
          const OFFSET: u8 = 0;
          const SHIFT: ShifterType = ShifterType::Shl(2);

          let inst = ptr::read_unaligned(p_rw as *mut u32);

          let (a, inst, _) = extract::<SIGNED, BITS>(inst, OFFSET, SHIFT);
          let dt = (s + a - p) >> 2;
          within_bits::<SIGNED, BITS>(dt)?;

          ptr::write_unaligned(p_rw as *mut u32, compact::<BITS>(inst, dt, OFFSET));
        }

        IMAGE_REL_ARM64_PAGEOFFSET_12A => {
          const BITS: u8 = 12;
          const SIGNED: bool = false;
          const OFFSET: u8 = 10;
          const SHIFT: ShifterType = ShifterType::None;

          let inst = ptr::read_unaligned(p_rw as *mut u32);

          let (a, inst, _) = extract::<SIGNED, BITS>(inst, OFFSET, SHIFT);
          let dt = (s + a) & 0xFFF;
          within_bits::<SIGNED, BITS>(dt)?;

          ptr::write_unaligned(p_rw as *mut u32, compact::<BITS>(inst, dt, OFFSET));
        }

        IMAGE_REL_ARM64_PAGEOFFSET_12L => {
          const BITS: u8 = 12;
          const SIGNED: bool = false;
          const OFFSET: u8 = 10;

          let inst = ptr::read_unaligned(p_rw as *mut u32);

          let scale = scale(inst);
          #[allow(non_snake_case)]
          let SHIFT: ShifterType = ShifterType::Shl(scale as _);

          let (a, inst, _) = extract::<SIGNED, BITS>(inst, OFFSET, SHIFT);

          let target = s + a;
          let page_offset = target & 0xFFF;
          if page_offset & ((1i64 << scale) - 1) != 0 {
            return Err(CoFFRError::Arm64AlignmentError(
              page_offset as u64,
              BITS as _,
            ));
          }

          let dt = page_offset >> scale;
          within_bits::<SIGNED, BITS>(dt)?;

          ptr::write_unaligned(p_rw as *mut u32, compact::<BITS>(inst, dt, OFFSET));
        }

        IMAGE_REL_ARM64_BRANCH19 => {
          const BITS: u8 = 19;
          const SIGNED: bool = true;
          const OFFSET: u8 = 5;
          const SHIFT: ShifterType = ShifterType::Shl(2);

          let inst = ptr::read_unaligned(p_rw as *mut u32);

          let (a, inst, _) = extract::<SIGNED, BITS>(inst, OFFSET, SHIFT);
          let dt = (s + a - p) >> 2;
          within_bits::<SIGNED, BITS>(dt)?;

          ptr::write_unaligned(p_rw as *mut u32, compact::<BITS>(inst, dt, OFFSET));
        }

        IMAGE_REL_ARM64_BRANCH14 => {
          const BITS: u8 = 14;
          const SIGNED: bool = true;
          const OFFSET: u8 = 5;
          const SHIFT: ShifterType = ShifterType::Shl(2);

          let inst = ptr::read_unaligned(p_rw as *mut u32);

          let (a, inst, _) = extract::<SIGNED, BITS>(inst, OFFSET, SHIFT);
          let dt = (s + a - p) >> 2;
          within_bits::<SIGNED, BITS>(dt)?;

          ptr::write_unaligned(p_rw as *mut u32, compact::<BITS>(inst, dt, OFFSET));
        }

        IMAGE_REL_ARM64_REL21 => {
          let inst = ptr::read_unaligned(p_rw as *mut u32);
          let a = extract_adr21(inst);
          let dt = s + a - p;
          within_bits::<true, 21>(dt)?;

          ptr::write_unaligned(p_rw as *mut u32, compact_adr21(inst, dt));
        }

        IMAGE_REL_ARM64_REL32 => {
          let a = ptr::read_unaligned(p_rw as *mut i32) as i64;
          let target = s + a - p;

          within_bits::<true, 32>(target)?;
          ptr::write_unaligned(p_rw as *mut i32, target as _);
        }

        IMAGE_REL_ARM64_PAGEBASE_REL21 => {
          let inst = ptr::read_unaligned(p_rw as *mut u32);
          let a = extract_adr21(inst) << 12;

          let target_page = (s + a) & !0xFFF;
          let cur_page = p & !0xFFF;
          let dt = (target_page - cur_page) >> 12;
          within_bits::<true, 21>(dt)?;

          ptr::write_unaligned(p_rw as *mut u32, compact_adr21(inst, dt));
        }

        reloc => return Err(CoFFRError::UnknownRelocation(reloc)),
      }
    }
  }

  Ok(())
}

fn extract_adr21(inst: u32) -> i64 {
  let immlo = (inst >> 29) & 0x3;
  let immhi = (inst >> 5) & 0x7FFFF;
  let raw = ((immhi << 2) | immlo) as i64;
  // Sign-extend 21 bits to 64 bits
  if (raw & (1 << 20)) != 0 {
    raw | !0x1FFFFF
  } else {
    raw
  }
}

fn compact_adr21(inst: u32, dt: i64) -> u32 {
  let imm21 = (dt as u32) & 0x1FFFFF;
  let immlo = imm21 & 0x3;
  let immhi = (imm21 >> 2) & 0x7FFFF;
  (inst & !(0x3 << 29 | 0x7FFFF << 5)) | (immlo << 29) | (immhi << 5)
}

fn scale(inst: u32) -> u32 {
  let mut size = inst >> 30;

  // https://llvm.org/docs/doxygen/RuntimeDyldCOFFAArch64_8h_source.html
  // line 51
  //
  // 0x04000000 indicates SIMD/FP registers
  // 0x00800000 indicates 128 bit
  if (inst & 0x04800000) == 0x04800000 {
    size += 4;
  }

  size
}

const fn extract<const SIGNED: bool, const BITS: u8>(
  data: u32,
  offset: u8,
  shifter: ShifterType,
) -> (i64, u32, u32) {
  let mask = maxint(false, BITS) as u32;
  let field = (data >> offset) & mask;

  let value = if SIGNED && BITS != 0 && field & (1 << (BITS - 1)) != 0 {
    (field as i64) | !(mask as i64)
  } else {
    field as i64
  };

  (shift(shifter, value), data & !(mask << offset), mask)
}

const fn compact<const BITS: u8>(inst: u32, value: i64, offset: u8) -> u32 {
  let mask = maxint(false, BITS) as u32;

  (inst & !(mask << offset)) | ((value as u32 & mask) << offset)
}

pub enum ShifterType {
  None,
  Shl(u8),
  Shr(u8),
}
const fn shift(data: ShifterType, val: i64) -> i64 {
  match data {
    ShifterType::None => val,
    ShifterType::Shl(n) => val << n,
    ShifterType::Shr(n) => val >> n,
  }
}

fn within_bits<const SIGNED: bool, const BITS: u8>(patch: i64) -> Result<(), CoFFRError> {
  assert!(BITS < 64);

  if !(minint(SIGNED, BITS)..=maxint(SIGNED, BITS)).contains(&patch) {
    return Err(CoFFRError::RelocationOverflow(patch as u64, BITS as _));
  }

  Ok(())
}

const fn minint(signed: bool, bits: u8) -> i64 {
  if !signed {
    return 0;
  }

  (!0i64 ^ (1 << (bits - 1))).wrapping_add(1)
}

const fn maxint(signed: bool, bits: u8) -> i64 {
  let mut acc = 0;

  let mut bit = 0;
  while bit < (bits - signed as u8) {
    acc |= 1 << bit;
    bit += 1;
  }

  acc
}
