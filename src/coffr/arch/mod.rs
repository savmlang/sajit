use std::{borrow::Borrow, iter};

use object::pe::RelocationType;

use crate::{
  MemoryExecutable, MemoryExecutableApi, SizeAlign, SizeCheck, WriteFnResult,
  coffr::{Arch, CoFFRError, Name, Relocation, Resolved, Section, SectionIdx, Symbol},
  platform::flush_icache,
  relcar::RELCAR_BASIC,
};

const KB_64: usize = 64 * 1024;

pub mod arm64;
pub mod x64;
pub mod x86;

#[inline]
pub(crate) fn link_binary<'data, 'transient, 'c, E, T, R1, R2>(
  memory: &'c mut MemoryExecutable,
  text: Section<R1>,
  rdata: Option<Section<R2>>,
  arch: Arch,
  symbols: T,
) -> Result<impl Iterator<Item = Result<(Name<'data>, u64), CoFFRError>> + 'transient, CoFFRError>
where
  'data: 'transient,
  E: Borrow<Symbol<'data>>,
  T: Iterator<Item = Result<E, CoFFRError>> + 'transient,
  R1: Iterator<Item = Result<Relocation, CoFFRError>>,
  R2: Iterator<Item = Result<Relocation, CoFFRError>>,
{
  if !memory
    .under_size_adv(
      [SizeAlign {
        size: text.data.len(),
        align: text.align as _,
      }]
      .into_iter()
      .chain(rdata.as_ref().map(|x| SizeAlign {
        size: x.data.len(),
        align: x.align as _,
      })),
    )
    .unwrap_or_default()
  {
    return Err(CoFFRError::UnderSized);
  }

  // Get an imagebase
  let imagebase = unsafe {
    let rx_base = memory.rxview as *const u8;
    let rx_base_cursor = rx_base.byte_add(memory.cursor);

    let neg_offset = (rx_base_cursor.addr()) % KB_64;
    rx_base_cursor.sub(neg_offset)
  };

  let mut rdata_ptr: Option<SectionView> = None;
  let text_ptr: SectionView;

  // Dump directly and findout pointer
  {
    if let Some(ref rdata) = rdata {
      unsafe {
        let rxptr = match memory.write_fn_iterated(
          rdata.align as _,
          rdata.data.len(),
          iter::once(rdata.data.as_ref()),
          [].iter(),
          &RELCAR_BASIC,
        ) {
          WriteFnResult::Executable(rx) => rx,
          WriteFnResult::OutOfSlab => return Err(CoFFRError::UnderSized),
        };

        // Now we shall fetch the RWPTR
        // The tradeoff is that we gotta flush again
        let cursor = rxptr.addr() - memory.rxview.addr();

        rdata_ptr = Some(SectionView {
          rw_ptr: (memory.rwview as *mut u8).add(cursor),
          rx_ptr: rxptr,
          len: rdata.data.len(),
        });
      }
    }

    text_ptr = unsafe {
      let rxptr = match memory.write_fn_iterated(
        text.align as _,
        text.data.len(),
        iter::once(text.data.as_ref()),
        [].iter(),
        &RELCAR_BASIC,
      ) {
        WriteFnResult::Executable(rx) => rx,
        WriteFnResult::OutOfSlab => return Err(CoFFRError::UnderSized),
      };

      // Now we shall fetch the RWPTR
      // The tradeoff is that we gotta flush again
      let cursor = rxptr.addr() - memory.rxview.addr();

      SectionView {
        rw_ptr: (memory.rwview as *mut u8).add(cursor),
        rx_ptr: rxptr,
        len: text.data.len(),
      }
    };
  }

  let symbolmap = |text_ptr: SectionView, rdata_ptr: Option<SectionView>, symbol: Resolved| {
    let mut sectstart = 0;
    let mut sectidx = 0;
    let resolved = match symbol {
      Resolved::Absolute(x) => x,
      Resolved::Section { idx, offset } => match idx {
        SectionIdx::Text(id) => {
          sectidx = id as u16;

          sectstart = text_ptr.rx_ptr.addr() as u64;
          text_ptr.rx_ptr.addr() as u64 + offset
        }
        SectionIdx::RData(id) => {
          sectidx = id as u16;

          let sect = rdata_ptr.ok_or(CoFFRError::InvalidObject)?;
          sectstart = sect.rx_ptr.addr() as u64;
          sect.rx_ptr.addr() as u64 + offset
        }
      },
    };

    Ok::<_, CoFFRError>((resolved, sectstart, sectidx))
  };

  let relocparse = |x: Result<Relocation, CoFFRError>| {
    let x = x?;
    let (symbol, sectionstart, sectidx) = symbolmap(text_ptr, rdata_ptr, x.symbol)?;
    let reloc = ResolvedRelocation {
      typ: x.typ,
      position_offset: x.position_offset,
      symbol,
      sectionstart,
      sectidx,
    };
    Ok::<_, CoFFRError>(reloc)
  };

  // Prime relocations
  let text_link = LinkSection {
    view: text_ptr,
    reloc: text.relocations.map(relocparse),
  };
  let rdata_link = rdata.zip(rdata_ptr).map(|(x, view)| LinkSection {
    view,
    reloc: x.relocations.map(relocparse),
  });

  // Relocate (Soon)
  match arch {
    Arch::X64 => x64::relocate(imagebase.addr() as _, text_link, rdata_link),
    Arch::X86 => x86::relocate(imagebase.addr() as _, text_link, rdata_link),
    Arch::Arm64 => arm64::relocate(imagebase.addr() as _, text_link, rdata_link),
  }?;

  // Flush Caches
  {
    if let Some(SectionView { rx_ptr, len, .. }) = rdata_ptr {
      _ = flush_icache(rx_ptr as _, len);
    }
    _ = flush_icache(text_ptr.rx_ptr as _, text_ptr.len);
  }

  Ok(symbols.map(move |x| {
    let binding = x?;
    let data = binding.borrow();

    Ok((data.name, symbolmap(text_ptr, rdata_ptr, data.resolved)?.0))
  }))
}

#[derive(Debug, Clone, Copy)]
pub struct SectionView {
  pub rw_ptr: *mut u8,
  pub rx_ptr: *const crate::Executable,
  pub len: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct LinkSection<T>
where
  T: Iterator<Item = Result<ResolvedRelocation, CoFFRError>>,
{
  pub view: SectionView,
  pub reloc: T,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedRelocation {
  pub typ: RelocationType,
  pub sectidx: u16,

  pub position_offset: u64,

  pub sectionstart: u64,
  pub symbol: u64,
}
