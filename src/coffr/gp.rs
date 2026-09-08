use object::{
  File,
  pe::{AnonObjectHeaderBigobj, ImageFileHeader},
};

use crate::{
  coffr::{CoFFR, CoFFRError, Name, cache::LinkerTransaction},
  transaction::MemoryTransaction,
};

pub enum GPCoffr<'a> {
  COFF(CoFFR<'a, ImageFileHeader>),
  COFFBig(CoFFR<'a, AnonObjectHeaderBigobj>),
}

impl<'a> GPCoffr<'a> {
  pub fn new(bin: &'a [u8]) -> Result<Self, CoFFRError> {
    Ok(match File::parse(bin)? {
      File::Coff(file) => Self::COFF(CoFFR { file }),
      File::CoffBig(file) => Self::COFFBig(CoFFR { file }),
      _ => return Err(CoFFRError::InvalidObject),
    })
  }

  pub fn link<'memory, 'output, 'data, R>(
    &'data self,
    resolve: &'output R,
    memory: MemoryTransaction<'memory>,
    vect: LinkerTransaction<'output>,
  ) -> Result<
    LinkIter<
      impl Iterator<Item = Result<(Name<'data>, u64), CoFFRError>> + 'output,
      impl Iterator<Item = Result<(Name<'data>, u64), CoFFRError>> + 'output,
    >,
    CoFFRError,
  >
  where
    // 'output describes the life of output stream
    // 'data is the life of the main structure
    // 'memory is the life of the memory executable
    //
    // For output, 'data must outlive 'output
    // 'memory ONLY needs to be valid until this function invocation
    'data: 'output,
    R: Fn(Name<'data>) -> u64 + 'output,
  {
    Ok(match self {
      Self::COFF(coff) => LinkIter::Standard(coff.link(resolve, memory, vect)?),
      Self::COFFBig(coff) => LinkIter::Big(coff.link(resolve, memory, vect)?),
    })
  }
}

pub enum LinkIter<I1, I2> {
  Standard(I1),
  Big(I2),
}

impl<I1, I2, T> Iterator for LinkIter<I1, I2>
where
  I1: Iterator<Item = T>,
  I2: Iterator<Item = T>,
{
  type Item = T;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Self::Standard(it) => it.next(),
      Self::Big(it) => it.next(),
    }
  }

  #[inline]
  fn size_hint(&self) -> (usize, Option<usize>) {
    match self {
      Self::Standard(it) => it.size_hint(),
      Self::Big(it) => it.size_hint(),
    }
  }
}
