use std::ptr;

use object::pe::{
  IMAGE_REL_AMD64_ABSOLUTE, IMAGE_REL_AMD64_ADDR32, IMAGE_REL_AMD64_ADDR32NB,
  IMAGE_REL_AMD64_ADDR64, IMAGE_REL_AMD64_REL32, IMAGE_REL_AMD64_REL32_1, IMAGE_REL_AMD64_REL32_2,
  IMAGE_REL_AMD64_REL32_3, IMAGE_REL_AMD64_REL32_4, IMAGE_REL_AMD64_REL32_5,
  IMAGE_REL_AMD64_SECREL, IMAGE_REL_AMD64_SECTION,
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

    let sect_start = relocation.sectionstart as i64;
    let s_idx = relocation.sectidx;
    let s = relocation.symbol as i64;
    let p = view.rx_ptr.addr() as i64 + relocation.position_offset as i64;

    let p_rw = unsafe { view.rw_ptr.add(relocation.position_offset as _) };

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

        bits32reloc => {
          let a = ptr::read_unaligned(p_rw as *mut i32) as i64;
          let (patch, signed) = match bits32reloc {
            IMAGE_REL_AMD64_ADDR32 => (s + a, false),
            IMAGE_REL_AMD64_ADDR32NB => ((s + a) - b, false),

            IMAGE_REL_AMD64_REL32 => ((s + a) - (p + 4), true),
            IMAGE_REL_AMD64_REL32_1 => ((s + a) - (p + 5), true),
            IMAGE_REL_AMD64_REL32_2 => ((s + a) - (p + 6), true),
            IMAGE_REL_AMD64_REL32_3 => ((s + a) - (p + 7), true),
            IMAGE_REL_AMD64_REL32_4 => ((s + a) - (p + 8), true),
            IMAGE_REL_AMD64_REL32_5 => ((s + a) - (p + 9), true),

            IMAGE_REL_AMD64_SECREL => (s - sect_start + a, true),
            reloc => return Err(CoFFRError::UnknownRelocation(reloc)),
          };

          println!(
            "P={p}, A={a}, S={s}, B={b}, SIdx={s_idx}, SectStart={sect_start}, Patch={patch}, Signed={signed}, Reloc={bits32reloc}"
          );

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
