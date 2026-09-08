use std::ptr;

use object::pe::{
  IMAGE_REL_I386_ABSOLUTE, IMAGE_REL_I386_DIR16, IMAGE_REL_I386_DIR32, IMAGE_REL_I386_DIR32NB,
  IMAGE_REL_I386_REL16, IMAGE_REL_I386_REL32, IMAGE_REL_I386_SECREL, IMAGE_REL_I386_SECREL7,
  IMAGE_REL_I386_SECTION, IMAGE_REL_I386_SEG12, IMAGE_REL_I386_TOKEN,
};

use crate::coffr::{
  CoFFRError,
  arch::{COFFRRelocator, CheckLinkSection, LinkSection},
};

pub(crate) struct X86Relocator;

impl COFFRRelocator for X86Relocator {
  fn relocate(&self, imagebase: u64, section: LinkSection) -> Result<(), CoFFRError> {
    unsafe {
      link(imagebase, section)?;

      Ok(())
    }
  }

  unsafe fn count_trampolines(
    &self,
    _section: CheckLinkSection,
  ) -> Result<(u32, &'static [u8]), CoFFRError> {
    Ok((0, &[]))
  }
}

unsafe fn link(base: u64, section: LinkSection) -> Result<(), CoFFRError> {
  let view = section.view;

  let b = base as i64;
  for relocation in section.reloc {
    let relocation = relocation?;

    let sect_start = relocation.sectionstart as i64;
    let s_idx = relocation.sectidx;
    let s = relocation.symbol as i64;
    let p = view.data.rx.addr() as i64 + relocation.position_offset as i64;

    let p_rw = unsafe { view.data.rw.add(relocation.position_offset as _) };

    unsafe {
      match relocation.typ {
        IMAGE_REL_I386_ABSOLUTE => continue,

        IMAGE_REL_I386_SECTION => {
          let a = ptr::read_unaligned(p_rw as *mut u16);
          let patch = s_idx + a;
          ptr::write_unaligned(p_rw as *mut u16, patch);
        }

        IMAGE_REL_I386_DIR16
        | IMAGE_REL_I386_REL16
        | IMAGE_REL_I386_SECREL7
        | IMAGE_REL_I386_SEG12
        | IMAGE_REL_I386_TOKEN => return Err(CoFFRError::UnknownRelocation(relocation.typ)),

        bits32 => {
          let a = ptr::read_unaligned(p_rw as *mut i32) as i64;
          let value = match bits32 {
            IMAGE_REL_I386_DIR32 => (s + a) & 0xFFFFFFFF,
            IMAGE_REL_I386_DIR32NB => s + a - b,
            IMAGE_REL_I386_SECREL => s - sect_start + a,
            IMAGE_REL_I386_REL32 => s + a - (p + 4),
            reloc => return Err(CoFFRError::UnknownRelocation(reloc)),
          };

          ptr::write_unaligned(p_rw as *mut u32, value as _);
        }
      }
    }
  }

  Ok(())
}
