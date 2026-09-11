use object::{File, Object, ObjectSection, RelocationKind, Section, SectionKind};

use crate::{MemoryExecutable, ObjectFileSizeCalc, SizeAlign};

mod arch;

impl ObjectFileSizeCalc for MemoryExecutable {
  fn sizecalc<T, Out>(object: &[u8], adapter: T) -> Result<Out, object::Error>
  where
    T: FnOnce(&mut dyn Iterator<Item = SizeAlign>) -> Out,
  {
    let file = File::parse(object)?;

    let stubsize = arch::get_stubsize(file.architecture());

    let sectfilter = |sect: &Section| match sect.kind() {
      SectionKind::Debug | SectionKind::DebugString | SectionKind::Metadata => false,
      _ => true,
    };

    file.sections().filter(sectfilter).try_for_each(|x| {
      x.compressed_data()?;
      Ok(())
    })?;

    let mut iterator = file
      .sections()
      .filter(sectfilter)
      .zip(std::iter::repeat(stubsize))
      .map(|(x, stubsize)| {
        let relocs =
          x.relocations()
            .zip(std::iter::repeat(stubsize))
            .filter_map(|((_, x), stubsize)| match x.kind() {
              RelocationKind::None
              | RelocationKind::Absolute
              | RelocationKind::SectionIndex
              | RelocationKind::SectionOffset => None,
              _ => Some(stubsize),
            });

        std::iter::once(SizeAlign {
          align: x.align() as _,
          size: x.compressed_data().unwrap().uncompressed_size as _,
        })
        .chain(relocs)
      })
      .flatten();

    Ok(adapter(&mut iterator))
  }
}
