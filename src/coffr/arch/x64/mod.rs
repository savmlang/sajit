use std::ptr;

use object::pe::{
  IMAGE_REL_AMD64_ABSOLUTE, IMAGE_REL_AMD64_ADDR32, IMAGE_REL_AMD64_ADDR32NB,
  IMAGE_REL_AMD64_ADDR64, IMAGE_REL_AMD64_REL32, IMAGE_REL_AMD64_REL32_1, IMAGE_REL_AMD64_REL32_2,
  IMAGE_REL_AMD64_REL32_3, IMAGE_REL_AMD64_REL32_4, IMAGE_REL_AMD64_REL32_5,
  IMAGE_REL_AMD64_SECREL, IMAGE_REL_AMD64_SECTION,
};

use crate::coffr::{
  CoFFRError, Resolved,
  arch::{COFFRRelocator, CheckLinkSection, LinkSection},
};

pub(crate) struct X64Relocator;

pub const X64_TRAMPOLINE_TEMPLATE: [u8; 16] = [
  0x66, 0x90, // nop, nop
  0xFF, 0x25, 0x00, 0x00, 0x00, 0x00, // jmp qword ptr [rip + 0]
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // target
];

impl COFFRRelocator for X64Relocator {
  fn relocate(&self, imagebase: u64, section: LinkSection) -> Result<(), CoFFRError> {
    unsafe {
      link(imagebase, section)?;

      Ok(())
    }
  }

  unsafe fn count_trampolines(
    &self,
    section: CheckLinkSection,
  ) -> Result<(u32, &'static [u8]), CoFFRError> {
    let mut count = 0u32;

    let view = section.view;
    for relocation in section.reloc {
      let relocation = relocation?;
      let s = match relocation.symbol {
        Resolved::Absolute(dt) => dt as i64,
        _ => continue,
      };

      let p_rw = unsafe { view.data.rw.add(relocation.position_offset as _) };

      unsafe {
        match relocation.typ {
          IMAGE_REL_AMD64_REL32 => {
            let a = ptr::read_unaligned(p_rw as *mut i32) as i64;

            let patch = s + a;

            if !(i32::MIN as i64..=i32::MAX as i64).contains(&patch) {
              count += 1;
            }
          }

          _ => continue,
        }
      }
    }

    Ok((count, &X64_TRAMPOLINE_TEMPLATE))
  }
}

unsafe fn link(base: u64, section: LinkSection) -> Result<(), CoFFRError> {
  let view = section.view;

  let b = base as i64;

  let mut trampoline = 0;
  for relocation in section.reloc {
    let relocation = relocation?;

    let sect_start = relocation.sectionstart as i64;
    let s_idx = relocation.sectidx;
    let s = relocation.symbol as i64;
    let p = view.data.rx.addr() as i64 + relocation.position_offset as i64;

    let p_rw = unsafe { view.data.rw.add(relocation.position_offset as _) };

    unsafe {
      match relocation.typ {
        IMAGE_REL_AMD64_ABSOLUTE => continue,
        IMAGE_REL_AMD64_ADDR64 => {
          let a = ptr::read_unaligned(p_rw as *mut i64);
          ptr::write_unaligned(p_rw as *mut u64, (s + a) as _);
        }
        IMAGE_REL_AMD64_SECTION => {
          let a = ptr::read_unaligned(p_rw as *mut u16);
          ptr::write_unaligned(p_rw as *mut u16, (s_idx + a) as _);
        }

        IMAGE_REL_AMD64_REL32 => {
          let a = ptr::read_unaligned(p_rw as *mut i32) as i64;

          let mut patch = (s + a) - (p + 4);

          let validate = |patch: i64| {
            (i32::MIN as i64..=i32::MAX as i64)
              .contains(&patch)
              .then_some(())
              .ok_or(CoFFRError::RelocationOverflow(patch as u64, 32))
          };

          validate(patch).map_or_else(
            |_| {
              let write = section
                .view
                .trampoline
                .rw
                .byte_add(trampoline * X64_TRAMPOLINE_TEMPLATE.len() + 8);
              ptr::write_unaligned(write as *mut u64, (s + a) as u64);

              let s = section
                .view
                .trampoline
                .rx
                .byte_add(trampoline * X64_TRAMPOLINE_TEMPLATE.len())
                .addr() as i64;
              patch = s - (p + 4);

              trampoline += 1;

              validate(patch)
            },
            |_| Ok(()),
          )?;

          ptr::write_unaligned(p_rw as *mut u32, patch as _);
        }

        bits32reloc => {
          let a = ptr::read_unaligned(p_rw as *mut i32) as i64;
          let (patch, signed) = match bits32reloc {
            IMAGE_REL_AMD64_ADDR32 => (s + a, false),
            IMAGE_REL_AMD64_ADDR32NB => ((s + a) - b, false),

            IMAGE_REL_AMD64_REL32_1 => ((s + a) - (p + 5), true),
            IMAGE_REL_AMD64_REL32_2 => ((s + a) - (p + 6), true),
            IMAGE_REL_AMD64_REL32_3 => ((s + a) - (p + 7), true),
            IMAGE_REL_AMD64_REL32_4 => ((s + a) - (p + 8), true),
            IMAGE_REL_AMD64_REL32_5 => ((s + a) - (p + 9), true),

            IMAGE_REL_AMD64_SECREL => (s - sect_start + a, true),
            reloc => return Err(CoFFRError::UnknownRelocation(reloc)),
          };

          if signed {
            if !(i32::MIN as i64..=i32::MAX as i64).contains(&patch) {
              return Err(CoFFRError::RelocationOverflow(patch as u64, 32));
            }
          } else if !(0..=u32::MAX as i64).contains(&patch) {
            return Err(CoFFRError::RelocationOverflow(patch as u64, 32));
          }

          ptr::write_unaligned(p_rw as *mut u32, patch as _);
        }
      }
    }
  }

  Ok(())
}
