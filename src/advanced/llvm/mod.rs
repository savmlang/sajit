pub mod jitlinkdry;

use std::{
  borrow::Cow,
  collections::HashMap,
  ffi::{c_char, c_void},
  iter,
  ptr::null_mut,
  slice::from_raw_parts,
  str,
};

use crate::{
  Executable, LLVMJITLink, MemoryExecutable, WriteFnResult,
  relcar::RELCAR_BASIC,
  relocations::llvmreloc::{
    AllocBlockSliceJL, AllocBlockSlicesJL, AllocRequestJL, RustMemoryInterfaceJL, create_linkctx,
    link_consume_linkctx,
  },
  symbpool::LLVMSymbolPool,
  transaction::MemoryTransaction,
};

#[cfg(target_os = "macos")]
#[link(name = "pthread")]
unsafe extern "C" {
  fn pthread_jit_write_protect_np(enabled: i32);
}

pub(crate) struct DataJITNote<'a, T: FnMut(*const str) -> usize> {
  pub mem: MemoryTransaction<'a>,
  #[cfg(windows)]
  pub rootaddr: usize,
  pub resolver: T,
  pub errors: Vec<Cow<'static, str>>,
  pub resolved: HashMap<Box<str>, *const Executable>,
}

impl LLVMJITLink for MemoryExecutable {
  fn write_jitlink<T>(
    &mut self,
    total: usize,
    symbolpool: &LLVMSymbolPool,
    object: &[u8],
    resolver: T,
  ) -> Result<HashMap<Box<str>, *const Executable>, Cow<'static, [std::borrow::Cow<'static, str>]>>
  where
    T: FnMut(*const str) -> usize,
  {
    let oldcursor = self.cursor;
    let mut data = DataJITNote {
      #[cfg(windows)]
      rootaddr: unsafe { self.rxview.byte_add(oldcursor).addr() },

      mem: MemoryTransaction::new(self, total),
      resolver,
      errors: vec![],
      resolved: HashMap::new(),
    };

    let mut rustmem = RustMemoryInterfaceJL {
      state: &mut data as *mut _ as _,
      allocateJIT: Some(allocate_jit::<T>),
      freeJITStructure: Some(free_jit),
      getfnPtr: Some(get_fn_ptr::<T>),
      onError: Some(on_err::<T>),
      storeAddr: Some(store_ptr::<T>),
    };

    unsafe {
      let ctx_ptr = create_linkctx(&mut rustmem);

      if link_consume_linkctx(
        ctx_ptr,
        symbolpool.symbpool.as_ptr(),
        object.as_ptr() as _,
        object.len(),
      ) != 0
      {
        return Err(Cow::Borrowed(&[Cow::Borrowed(
          "Could not link context pointer",
        )]));
      }
    }

    if data.errors.is_empty() {
      data.mem.commit();

      return Ok(data.resolved);
    }

    Err(Cow::Owned(data.errors))
  }
}

unsafe extern "C" fn store_ptr<T>(
  state: *mut c_void,
  ptr: *const c_char,
  len: usize,
  store_addr: u64,
) where
  T: FnMut(*const str) -> usize,
{
  unsafe {
    let state = &mut *(state as *mut DataJITNote<T>);

    if let Ok(err) = str::from_utf8(from_raw_parts(ptr as *const u8, len)) {
      // Replacement is expected
      _ = state.resolved.insert(
        Box::from(err),
        store_addr as usize as *const () as *const Executable,
      );
    }
  }
}

unsafe extern "C" fn on_err<T>(state: *mut c_void, ptr: *const c_char)
where
  T: FnMut(*const str) -> usize,
{
  use std::ffi::CStr;

  unsafe {
    let state = &mut *(state as *mut DataJITNote<T>);

    let cstr = CStr::from_ptr(ptr);

    let err = cstr.to_string_lossy().into_owned();
    state.errors.push(Cow::Owned(err));
  }
}

unsafe extern "C" fn get_fn_ptr<T>(state: *mut c_void, ptr: *const c_char, size: usize) -> usize
where
  T: FnMut(*const str) -> usize,
{
  (|| unsafe {
    let state = &mut *(state as *mut DataJITNote<T>);

    let string = str::from_utf8(from_raw_parts(ptr as *const u8, size)).ok()?;

    #[cfg(windows)]
    if string == "__ImageBase" {
      return Some(state.rootaddr);
    }

    Some((state.resolver)(string))
  })()
  .unwrap_or_default()
}

unsafe extern "C" fn free_jit(_: *mut c_void, req: AllocBlockSlicesJL) {
  if !req.allocs.is_null() {
    unsafe {
      let slice = std::ptr::slice_from_raw_parts_mut(req.allocs, req.len);
      drop(Box::from_raw(slice));
    };
  }
}

unsafe extern "C" fn allocate_jit<T>(
  state: *mut c_void,
  req: *mut AllocRequestJL,
  len: usize,
) -> AllocBlockSlicesJL
where
  T: FnMut(*const str) -> usize,
{
  unsafe {
    let state = state as *mut DataJITNote<T>;
    let trans = &mut (*state).mem;

    let mut out = AllocBlockSlicesJL {
      allocs: null_mut(),
      len: 0,
    };

    let allocation = from_raw_parts(req, len);
    let allocobj = allocation
      .into_iter()
      .map(|alloc| {
        let rx = match trans.write_fn_iterated::<false, _, _, _, _>(
          alloc.alignment as _,
          alloc.size,
          iter::empty::<&[u8]>(),
          iter::empty::<crate::relocations::Relocation>(),
          &RELCAR_BASIC,
        ) {
          WriteFnResult::Executable(ex) => ex.addr(),
          _ => return None,
        };

        let cursor = rx - trans.get_mut().rxview.addr();

        Some(AllocBlockSliceJL {
          rwview: trans.get_mut().rwview.addr() + cursor,
          rxview: trans.get_mut().rxview.addr() + cursor,
        })
      })
      .collect::<Option<Box<[_]>>>();

    if let Some(allocobj) = allocobj {
      out.len = allocobj.len();
      out.allocs = Box::into_raw(allocobj) as _;
    }

    out
  }
}
