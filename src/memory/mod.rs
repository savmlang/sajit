use std::{
  borrow::Borrow,
  iter::{empty, once},
  mem,
  num::NonZeroU8,
  pin::Pin,
  sync::atomic::AtomicUsize,
};

use crate::{
  Executable, MemoryExecutable, MemoryExecutableApi, SizeAlign, SizeCheck, WriteFnResult,
  relcar::RELCAR_BASIC, relocations::Relocation, transaction::MemoryTransaction,
};

pub type PinnedMemExec = Pin<Box<MemoryExecutable>>;

pub struct DynamicMemory<E>
where
  E: Borrow<[u8]>,
{
  execs: Vec<PinnedMemExec>,
  init: E,
}

impl<E: Borrow<[u8]>> DynamicMemory<E> {
  pub fn new(init: E) -> Self {
    Self {
      execs: Vec::new(),
      init,
    }
  }

  pub fn gc(&mut self) {
    let mut i = self.execs.len();
    while i > 0 {
      i -= 1;

      let should_free = unsafe { self.execs.get_unchecked_mut(i).try_free().is_ok() };

      if should_free {
        let dt = self.execs.swap_remove(i);
        let mexec = *Pin::into_inner(dt);
        mem::forget(mexec);
      }
    }
  }

  fn alloc(&mut self, size: usize) {
    let init = self.init.borrow();

    let needed = size
      .checked_add(init.len())
      .and_then(|s| s.checked_add(32))
      .expect("Allocation size arithmetic overflow");

    let multiple_raw = needed.div_ceil(MemoryExecutable::DEFAULT_SLAB_SIZE).max(1);

    let multiple =
      NonZeroU8::new(u8::try_from(multiple_raw).expect("Slab allocation units exceed u8::MAX"));

    let mut slab = MemoryExecutable::new_slab(multiple);

    unsafe {
      _ = slab.write_fn_iterated::<false, true, _, _, _, _>(
        32,
        init.len(),
        once(init),
        empty::<Relocation>(),
        &RELCAR_BASIC,
      );
    }

    self.execs.push(Box::pin(slab));
  }

  pub unsafe fn process<T, DFn: FnMut() -> T, ProcessFn>(
    &mut self,
    total_functions: usize,
    mut prereq: DFn,
    process: ProcessFn,
  ) -> Result<ExecInfo, WriteFnResult>
  where
    T: Iterator<Item = SizeAlign>,
    ProcessFn: FnOnce(&mut MemoryTransaction<'_>) -> WriteFnResult,
  {
    let transact = |record: &mut MemoryExecutable| {
      let mut transaction = MemoryTransaction::new(record, total_functions);

      match process(&mut transaction) {
        WriteFnResult::Executable(ex) => {
          transaction.commit();

          Ok(ExecInfo {
            executable: ex,
            recordptr: AtomicUsizeWrapper(record.stored.as_ptr()),
          })
        }
        out => Err(out),
      }
    };

    if let Some(record) = self
      .execs
      .iter_mut()
      .find(|x| x.under_size_adv(prereq()).unwrap_or_default())
    {
      return transact(record);
    }

    let size = prereq()
      .map(|x| x.size + x.align.saturating_sub(1))
      .sum::<usize>();

    self.alloc(size);

    let idx = self.execs.len() - 1;
    unsafe { transact(self.execs.get_unchecked_mut(idx)) }
  }
}

pub struct ExecInfo {
  pub executable: *const Executable,
  pub recordptr: AtomicUsizeWrapper,
}

pub struct AtomicUsizeWrapper(*mut usize);

impl AtomicUsizeWrapper {
  pub unsafe fn deref(&self) -> &AtomicUsize {
    unsafe { AtomicUsize::from_ptr(self.0) }
  }
}
