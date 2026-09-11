use std::{
  borrow::Borrow,
  iter,
  ptr::{null, null_mut},
};

use object::{
  Object, ObjectSection,
  coff::{CoffFile, CoffHeader},
  pe::RelocationType,
};

use crate::{
  Executable,
  coffr::{
    Arch, CoFFRError, Name, Relocation, Resolved, Resultify, SectionIdx, Symbol,
    arch::{arm64::Arm64Relocator, x64::X64Relocator, x86::X86Relocator},
    cache::LinkerTransaction,
    relocparser,
  },
  relcar::RELCAR_BASIC,
  transaction::MemoryTransaction,
};

const KB_64: usize = 64 * 1024;

pub mod arm64;
pub mod x64;
pub mod x86;

#[inline]
pub(crate) fn link_binary<'data, 'transient, 'c, 'd, E, S: CoffHeader, T, R>(
  mut memory: MemoryTransaction<'d>,
  file: &'data CoffFile<'data, &'data [u8], S>,
  arch: Arch,
  symbols: T,
  mut sectview: LinkerTransaction<'transient>,
  resolve: &'transient R,
) -> Result<impl Iterator<Item = Result<(Name<'data>, u64), CoFFRError>> + 'transient, CoFFRError>
where
  'data: 'transient,
  E: Borrow<Symbol<'data>>,
  T: Iterator<Item = Result<E, CoFFRError>> + 'transient,
  R: Fn(Name<'data>) -> u64 + 'transient,
{
  let relocator: &'static dyn COFFRRelocator = match arch {
    Arch::Arm64 => &Arm64Relocator,
    Arch::X64 => &X64Relocator,
    Arch::X86 => &X86Relocator,
  };

  // Get an imagebase
  let imagebase = unsafe {
    let rx_base = memory.get_mut().rxview as *const u8;
    let rx_base_cursor = rx_base.byte_add(memory.get_mut().cursor);

    let neg_offset = (rx_base_cursor.addr()) % KB_64;
    rx_base_cursor.sub(neg_offset)
  };

  let symbolmap = |sectview: &LinkerTransaction, rxview: usize, symbol: Resolved| {
    let mut sectstart = 0;
    let mut sectidx = 0;

    let resolved = match symbol {
      Resolved::Absolute(x) => x,
      Resolved::Section { idx, offset } => {
        let id = match idx {
          SectionIdx::Text(id) => id,
          SectionIdx::RData(id) => id,
        };

        let view = sectview.get(id as _).resultify()?;

        sectstart = rxview as u64 + view.cursor as u64;
        sectidx = id as _;

        sectstart + offset
      }
    };

    Ok::<_, CoFFRError>((resolved, sectstart, sectidx))
  };

  let relocparse =
    |sectview: &LinkerTransaction, rxview: usize, x: Result<Relocation, CoFFRError>| {
      let x = x?;
      let (symbol, sectionstart, sectidx) = symbolmap(sectview, rxview, x.symbol)?;
      let reloc = ResolvedRelocation {
        typ: x.typ,
        position_offset: x.position_offset,
        symbol,
        sectionstart,
        sectidx,
      };
      Ok::<_, CoFFRError>(reloc)
    };

  // Put all "Tracking" sections first!
  for section in file
    .sections()
    .filter(|x| {
      x.name_bytes()
        .map(|x| x.starts_with(b".rdata") || x.starts_with(b".text"))
        .unwrap_or_default()
    })
    .map(|x| unsafe {
      let bin = x.uncompressed_data()?;
      let align = x.align();

      let rxptr = memory
        .write_fn_iterated::<true, _, _, _, _>(
          align as _,
          bin.len(),
          iter::once(bin.as_ref()),
          [].iter(),
          &RELCAR_BASIC,
        )
        .resultify()?;

      let cursor = rxptr.addr() - memory.get_mut().rxview.addr();

      let mut compressed = CompressedSectionView {
        trampolines: 0,
        len: bin.len() as _,

        cursor: u32::try_from(cursor).map_err(|_| CoFFRError::ConvertU32Err)?,
        cursor_trampolines: 0,
      };

      let (trampolines, cursor_trampolines) = {
        let mut reloc = relocparser(file, x.relocations(), resolve);
        let section = CheckLinkSection {
          view: SectionView {
            trampolines: compressed.trampolines,
            len: compressed.len,
            trampoline: RWRXPtr {
              rw: null_mut(),
              rx: null(),
            },
            data: RWRXPtr {
              rw: memory.get_mut().rwview.add(compressed.cursor as _),
              rx: memory.get_mut().rxview.add(compressed.cursor as _),
            },
          },
          reloc: &mut reloc,
        };

        let (count, template) = relocator.count_trampolines(section)?;
        let rxptr = memory
          .write_fn_iterated::<true, _, _, _, _>(
            16,
            template.len() * count as usize,
            iter::repeat_n(template, count as _),
            iter::empty::<crate::relocations::Relocation>(),
            &RELCAR_BASIC,
          )
          .resultify()?;

        let cursor = rxptr.addr() - memory.get_mut().rxview.addr();

        (count as u32, cursor as u32)
      };

      compressed.cursor_trampolines = cursor_trampolines;
      compressed.trampolines = trampolines;

      Ok::<_, CoFFRError>((x.index().0 as u32, compressed))
    })
  {
    let (key, item) = section?;
    sectview.insert_sorted(key, item);
  }

  for section in file.sections().filter(|x| {
    x.name_bytes()
      .map(|x| x.starts_with(b".rdata") || x.starts_with(b".text"))
      .unwrap_or_default()
  }) {
    let idx = section.index().0;
    let view = sectview.get(idx as _).resultify()?;

    let begin = memory.get_mut().rxview.addr();
    let mut reloc =
      relocparser(file, section.relocations(), resolve).map(|x| relocparse(&sectview, begin, x));
    let section = unsafe {
      LinkSection {
        view: SectionView {
          trampolines: view.trampolines,
          len: view.len,
          trampoline: RWRXPtr {
            rw: memory
              .get_mut()
              .rwview
              .byte_add(view.cursor_trampolines as _),
            rx: memory
              .get_mut()
              .rxview
              .byte_add(view.cursor_trampolines as _),
          },
          data: RWRXPtr {
            rw: memory.get_mut().rwview.byte_add(view.cursor as _),
            rx: memory.get_mut().rxview.byte_add(view.cursor as _),
          },
        },
        reloc: &mut reloc,
      }
    };

    relocator.relocate(imagebase.addr() as _, section)?;
  }

  // Commit to memory so that its not overwritten
  let rxview = memory.get_mut().rxview.addr();
  memory.commit();
  Ok(symbols.map(move |x| {
    let binding = x?;
    let data = binding.borrow();
    Ok((data.name, symbolmap(&sectview, rxview, data.resolved)?.0))
  }))
}

#[derive(Debug, Clone, Copy)]
pub struct CompressedSectionView {
  pub trampolines: u32,
  pub len: u32,

  pub cursor_trampolines: u32,
  pub cursor: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct SectionView {
  pub trampolines: u32,
  pub len: u32,

  pub trampoline: RWRXPtr,
  pub data: RWRXPtr,
}

#[derive(Debug, Clone, Copy)]
pub struct RWRXPtr {
  pub rw: *mut u8,
  pub rx: *const Executable,
}

pub struct LinkSection<'a> {
  pub view: SectionView,
  pub reloc: &'a mut dyn Iterator<Item = Result<ResolvedRelocation, CoFFRError>>,
}

pub struct CheckLinkSection<'a> {
  pub view: SectionView,
  pub reloc: &'a mut dyn Iterator<Item = Result<Relocation, CoFFRError>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedRelocation {
  pub typ: RelocationType,
  pub sectidx: u16,

  pub position_offset: u64,

  pub sectionstart: u64,
  pub symbol: u64,
}

pub(crate) trait COFFRRelocator {
  unsafe fn count_trampolines(
    &self,
    section: CheckLinkSection<'_>,
  ) -> Result<(u32, &'static [u8]), CoFFRError>;

  fn relocate(&self, imagebase: u64, section: LinkSection) -> Result<(), CoFFRError>;
}
