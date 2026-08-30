use std::borrow::Cow;

use object::{
  Architecture, File, Object, ObjectSection, ObjectSymbol, RelocationFlags, RelocationTarget,
  SymbolIndex, SymbolSection,
  coff::{CoffHeader, CoffRelocationIterator, CoffSection},
  pe::{AnonObjectHeaderBigobj, ImageFileHeader, RelocationType},
  read::coff::CoffFile,
};

use crate::{MemoryExecutable, coffr::arch::link_binary};

pub mod arch;

pub struct CoFFR<'a, T: CoffHeader> {
  pub file: CoffFile<'a, &'a [u8], T>,
}

pub trait SaJITCoffType {
  type Header: CoffHeader;

  fn parse<'a>(bin: &'a [u8]) -> Result<CoFFR<'a, Self::Header>, CoFFRError>;
}

pub struct SaJITCoffNormal;

impl SaJITCoffType for SaJITCoffNormal {
  type Header = ImageFileHeader;

  fn parse<'a>(bin: &'a [u8]) -> Result<CoFFR<'a, Self::Header>, CoFFRError> {
    match File::parse(bin)? {
      File::Coff(file) => return Ok(CoFFR { file }),
      _ => return Err(CoFFRError::InvalidObject),
    };
  }
}

pub struct SaJITCoffBig;

impl SaJITCoffType for SaJITCoffBig {
  type Header = AnonObjectHeaderBigobj;

  fn parse<'a>(bin: &'a [u8]) -> Result<CoFFR<'a, Self::Header>, CoFFRError> {
    match File::parse(bin)? {
      File::CoffBig(file) => return Ok(CoFFR { file }),
      _ => return Err(CoFFRError::InvalidObject),
    };
  }
}

impl<'a, T: CoffHeader> CoFFR<'a, T> {
  pub fn new(bin: &'a [u8]) -> Result<CoFFR<'a, ImageFileHeader>, CoFFRError> {
    Self::new_with::<SaJITCoffNormal>(bin)
  }

  pub fn new_with<E: SaJITCoffType>(bin: &'a [u8]) -> Result<CoFFR<'a, E::Header>, CoFFRError> {
    E::parse(bin)
  }

  pub fn link<'memory, 'output, 'data, R>(
    &'data self,
    resolve: &'output R,
    memory: &'memory mut MemoryExecutable,
  ) -> Result<impl Iterator<Item = Result<(Name<'data>, u64), CoFFRError>> + 'output, CoFFRError>
  where
    // 'output describes the life of output stream
    // 'data is the life of the main structure
    // 'memory is the life of the memory executable
    //
    // For output, 'data must outlive 'output
    // 'memory ONLY needs to be valid until this function invocation
    'data: 'output,
    R: Fn(Name) -> u64,
  {
    // Map the two sections
    let mut text = None;
    let mut rdata = None;

    for section in self.file.sections() {
      let name = section.name()?;

      fn fndata<'a, R, H: CoffHeader>(
        file: &'a CoffFile<'a, &'a [u8], H>,
        section: CoffSection<'a, 'a, &'a [u8], H>,
        resolve: &'a R,
      ) -> Result<Section<'a, impl Iterator<Item = Result<Relocation, CoFFRError>> + 'a>, CoFFRError>
      where
        R: Fn(Name<'a>) -> u64,
      {
        let data = section.uncompressed_data()?;

        let relocations = relocparser(&file, section.relocations(), resolve);
        Ok::<_, CoFFRError>(Section {
          data,
          align: section.align(),
          relocations,
        })
      }

      match name {
        ".text" => {
          text = Some(fndata(&self.file, section, resolve)?);
        }
        ".rdata" => {
          rdata = Some(fndata(&self.file, section, resolve)?);
        }
        ".bss" | ".data" => {
          assert!(
            section.compressed_data()?.uncompressed_size == 0,
            "LinkerError: {name} should be empty"
          );
        }
        _ => continue,
      }
    }

    let Some(text) = text else {
      assert!(false, ".text shouldn't be absent");
      unreachable!();
    };

    let arch = match self.file.architecture() {
      Architecture::Aarch64 => Arch::Arm64,
      Architecture::X86_64 => Arch::X64,
      Architecture::I386 => Arch::X86,
      arch => unreachable!("Unsupported COFF Architecture: {arch:?}"),
    };

    link_binary(
      memory,
      text,
      rdata,
      arch,
      self
        .file
        .symbols()
        .map(|x| resolve_symbol(&self.file, x.index(), resolve)),
    )
  }
}

fn resolve_symbol<'a, H: 'a + CoffHeader, R>(
  file: &'a CoffFile<'a, &'a [u8], H>,
  idx: SymbolIndex,
  resolve: &R,
) -> Result<Symbol<'a>, CoFFRError>
where
  R: Fn(Name<'a>) -> u64,
{
  let symbol = file.symbol_by_index(idx)?;
  let name_bytes = symbol.name_bytes()?;
  let name = str::from_utf8(name_bytes).map_or_else(|_| Name::Bytes(name_bytes), Name::UTF8);

  let resolved = match symbol.section() {
    SymbolSection::Absolute => Ok(Resolved::Absolute(symbol.address())),
    SymbolSection::None => Err(CoFFRError::InvalidObject),
    SymbolSection::Undefined => Ok(Resolved::Absolute(resolve(name))),
    SymbolSection::Section(id) => {
      let section = file.section_by_index(id)?;
      let name_bytes = section.name_bytes()?;
      let idx = match name_bytes {
        b".text" => SectionIdx::Text(id.0),
        b".rdata" => SectionIdx::RData(id.0),
        _ => return Err(CoFFRError::RelocsOutsideTextData),
      };
      Ok(Resolved::Section {
        idx,
        offset: symbol.address(),
      })
    }
    SymbolSection::Common => unimplemented!("SaJIT cannot use Common Sections"),
    e => unimplemented!("Unsupported SymbolSection: {e:?}"),
  };

  Ok(Symbol {
    name,
    resolved: resolved?,
  })
}

fn relocparser<'a, T: 'a + CoffHeader, R: Fn(Name<'a>) -> u64>(
  file: &'a CoffFile<'a, &'a [u8], T>,
  relocation: CoffRelocationIterator<'a, 'a, &'a [u8], T>,
  resolve: &'a R,
) -> impl Iterator<Item = Result<Relocation, CoFFRError>> + 'a {
  relocation.map(|(offset, reloc)| {
    let symbol = match reloc.target() {
      RelocationTarget::Absolute => ABS_RESOLVED,
      RelocationTarget::Symbol(idx) => resolve_symbol(file, idx, resolve)?.resolved,
      RelocationTarget::Section(idx) => {
        let name = file.section_by_index(idx)?.name_bytes()?;

        let idx = match name {
          b".text" => SectionIdx::Text(idx.0),
          b".rdata" => SectionIdx::RData(idx.0),
          _ => return Err(CoFFRError::RelocsOutsideTextData),
        };
        Resolved::Section { idx, offset: 0 }
      }

      e => unimplemented!("Unsupported {e:?}"),
    };

    let reloctype = match reloc.flags() {
      RelocationFlags::Coff { typ } => typ,
      e => unimplemented!("Unsupported: {e:?}"),
    };

    Ok(Relocation {
      position_offset: offset,
      typ: reloctype,
      symbol,
    })
  })
}

#[derive(Debug)]
pub enum CoFFRError {
  RelocsOutsideTextData,
  InvalidObject,
  UnderSized,
  UnknownRelocation(RelocationType),
  Arm64AlignmentError(u64, u64),
  // offset, width
  RelocationOverflow(u64, u64),
  Object(object::Error),
}

impl From<object::Error> for CoFFRError {
  fn from(value: object::Error) -> Self {
    Self::Object(value)
  }
}

pub struct Section<'a, T>
where
  T: Iterator<Item = Result<Relocation, CoFFRError>> + 'a,
{
  pub data: Cow<'a, [u8]>,
  pub align: u64,
  pub relocations: T,
}

impl<'a, T> std::fmt::Debug for Section<'a, T>
where
  T: Iterator<Item = Result<Relocation, CoFFRError>>,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Section")
      .field("data", &self.data)
      .field("align", &self.align)
      .finish_non_exhaustive()
  }
}

const ABS_RESOLVED: Resolved = Resolved::Absolute(0);

#[derive(Debug, Clone, Copy)]
pub enum Arch {
  Arm64,
  X64,
  X86,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Name<'a> {
  UTF8(&'a str),
  Bytes(&'a [u8]),
}

#[derive(Debug, Clone, Copy)]
pub struct Symbol<'a> {
  pub name: Name<'a>,
  pub resolved: Resolved,
}

#[derive(Debug, Clone, Copy)]
pub enum Resolved {
  Absolute(u64),
  Section { idx: SectionIdx, offset: u64 },
}

#[derive(Debug, Clone, Copy)]
pub enum SectionIdx {
  Text(usize),
  RData(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct Relocation {
  pub typ: RelocationType,

  // P
  pub position_offset: u64,
  // S
  pub symbol: Resolved,
  // A <-- To fetch later
}
