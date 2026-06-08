use core::slice;
use std::{collections::HashMap, ptr};

use windows::Win32::System::SystemServices::{IMAGE_REL_I386_DIR32, IMAGE_REL_I386_REL32};

use crate::{MemoryExecutable, coffr::coff::IMAGE_RELOCATION};

use super::coff::Coff;

pub const COFF_MAX_SYMBOLS: usize = 600;

pub struct I686COFFRelocator<'a, 'b> {
  coff: Coff<'a>,
  mem: &'b mut MemoryExecutable,
}

impl<'a, 'b> I686COFFRelocator<'a, 'b> {
  pub fn load<T>(buf: &'a T, mem: &'b mut MemoryExecutable) -> Result<Self, ()>
  where
    T: AsRef<[u8]> + 'a,
  {
    let coff = Coff::parse(buf.as_ref())?;

    Ok(Self { coff, mem })
  }

  pub unsafe fn prepare<F, E>(self, mut resolver: F, mut export_callback: E) -> Option<()>
  where
    F: FnMut(&str) -> u32,
    E: FnMut(&str, u32) -> (),
  {
    unsafe {
      let restore_cursor = self.mem.cursor;

      let mut cursor = self.mem.rwview.addr() + self.mem.cursor;
      let cursor_max = self.mem.rwview.addr() + self.mem.size;

      let signal = self.coff.sections.iter().enumerate();

      let mut symbols = HashMap::new();
      {
        let mut cursor = cursor;
        signal.clone().for_each(|(section_index, x)| {
          if x.SizeOfRawData == 0 {
            return;
          }

          let size = x.SizeOfRawData as usize;
          let align = Coff::get_section_alignment(x);

          cursor = cursor.next_multiple_of(align);
          let base = cursor;

          let ofset = base - self.mem.rwview.addr();
          let base_rx = self.mem.rxview.addr() + ofset;

          for symbol in &self.coff.symbols {
            if symbol.SectionNumber == (section_index + 1) as i16 {
              let func_name = self.coff.get_symbol_name(symbol);
              let func_runtime_address = (base_rx as u32) + symbol.Value;

              if symbol.StorageClass == 2 && Coff::is_fcn(symbol.Type) {
                export_callback(func_name.as_ref(), func_runtime_address);
              }

              symbols.insert(func_name, func_runtime_address);
            }
          }

          cursor += size;
        });
      };

      // Ignore empty sections and process
      let signal = signal
        .filter(|(_, x)| x.SizeOfRawData != 0)
        .map(|(idx, x)| {
          let sectname = Coff::get_section_name(x).expect("Unable to get section data");
          let relocs = self.coff.get_relocations(x);

          let ptr = self.coff.buffer.as_ptr().add(x.PointerToRawData as _);
          let mc = core::slice::from_raw_parts(ptr, x.SizeOfRawData as _);
          let align = Coff::get_section_alignment(x);

          (idx, sectname, relocs, mc, align)
        })
        .try_for_each(|(_, _, relocations, machinecode, align)| {
          // Align Cursor
          cursor = cursor.next_multiple_of(align);
          let base = cursor;

          if base.saturating_add(machinecode.len()) > cursor_max {
            return None;
          }

          // The of-set (not, offet) from RWVIEW
          let ofset = base - self.mem.rwview.addr();

          let base_rx = self.mem.rxview.addr() + ofset;
          let baseptr = self.mem.rwview.add(ofset);
          // Write to RWVIEW
          ptr::copy_nonoverlapping(machinecode.as_ptr(), baseptr, machinecode.len());

          let mutable_slice = slice::from_raw_parts_mut(baseptr, machinecode.len());

          for reloc in &relocations {
            apply_relocations(
              base_rx,
              &self.coff,
              reloc,
              mutable_slice.as_mut_ptr(),
              &symbols,
              &mut resolver,
            );
          }

          // Prepend cursor
          cursor += machinecode.len();

          Some(())
        });

      if let None = signal {
        self.mem.cursor = restore_cursor;
        return None;
      }

      self.mem.cursor = cursor - self.mem.rwview.addr();
      Some(())
    }
  }
}

fn apply_relocations<F>(
  base_rx: usize,
  coff: &Coff,
  relocation: &IMAGE_RELOCATION,
  data: *mut u8,

  map: &HashMap<Box<str>, u32>,
  resolver: &mut F,
) where
  F: FnMut(&str) -> u32,
{
  unsafe {
    let symbol = &coff.symbols[relocation.SymbolTableIndex as usize];

    let name = coff.get_symbol_name(symbol);
    let target_address = map
      .get(&name)
      .map(Clone::clone)
      .unwrap_or_else(|| resolver(name.as_ref()));

    let offset = relocation.Anonymous.VirtualAddress as usize;
    let site = data.add(offset as _) as *mut u32;

    match relocation.Type as u32 {
      IMAGE_REL_I386_DIR32 => {
        // Read what's currently there (often an offset or 0), add the real address
        let existing_offset = site.read_unaligned();

        site.write_unaligned(existing_offset.wrapping_add(target_address as u32));
      }
      IMAGE_REL_I386_REL32 => {
        let pc_address = (base_rx + offset + 4) as u32;

        let existing_offset = site.read_unaligned();

        let relative_offset = (target_address as u32)
          .wrapping_sub(pc_address)
          .wrapping_add(existing_offset);

        site.write_unaligned(relative_offset);
      }
      _ => unimplemented!(),
    }
  }
}
