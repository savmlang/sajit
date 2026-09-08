use std::{borrow::Borrow, mem::forget, sync::atomic::Ordering};

use crate::{
  MemoryExecutable, MemoryExecutableApi, WriteFnResult,
  platform::flush_icache,
  relcar::{Relcar, Relocator},
  relocations::Relocation,
};

pub struct MemoryTransaction<'memory> {
  memory: &'memory mut MemoryExecutable,

  total: usize,
  cursor: usize,
  writes: usize,
}

impl<'a> MemoryTransaction<'a> {
  pub fn new(memory: &'a mut MemoryExecutable, total_functions: usize) -> Self {
    Self {
      cursor: memory.cursor,
      memory,
      writes: 0,
      total: total_functions,
    }
  }

  pub fn get_mut(&mut self) -> &mut MemoryExecutable {
    self.memory
  }

  pub unsafe fn write_fn_iterated<
    'b,
    const WRITE: bool,
    T: Iterator<Item = &'b [u8]>,
    E: Iterator<Item = R>,
    R: Borrow<Relocation>,
    B: Relocator,
  >(
    &mut self,
    alignment: usize,
    capped_size: usize,
    data: T,
    relocs: E,
    relcar: &Relcar<B>,
  ) -> WriteFnResult {
    match unsafe {
      self.memory.write_fn_iterated::<false, WRITE, _, _, _, _>(
        alignment,
        capped_size,
        data,
        relocs,
        relcar,
      )
    } {
      WriteFnResult::Executable(ex) => {
        self.writes += 1;
        WriteFnResult::Executable(ex)
      }
      e => e,
    }
  }

  pub fn commit(self) {
    self.memory.stored.fetch_add(self.total, Ordering::Relaxed);

    // Auto flush it all
    unsafe {
      let cur = self.memory.cursor;

      flush_icache(
        self.memory.rxview.add(self.cursor) as _,
        cur.strict_sub(self.cursor),
      );
    }

    forget(self);
  }
}

impl<'a> Drop for MemoryTransaction<'a> {
  fn drop(&mut self) {
    self.memory.cursor = self.cursor;

    if self.writes > 0 {
      self.memory.stored.fetch_sub(self.writes, Ordering::Release);
    }
  }
}
