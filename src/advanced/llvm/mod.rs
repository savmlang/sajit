pub mod rtdyld;

use std::{
  borrow::Cow, collections::HashMap, ffi::c_void, ptr::null_mut, slice::from_raw_parts, str,
  sync::atomic::Ordering,
};

use crate::{
  Executable, LLVMJITLink, MemoryExecutable,
  relocations::llvmreloc::jitlink::{
    AllocBlockSlice, AllocBlockSlices, AllocRequest, RustMemoryInterface, create_linkctx,
    link_consume_linkctx,
  },
  symbpool::LLVMSymbolPool,
};

#[cfg(target_os = "macos")]
#[link(name = "pthread")]
unsafe extern "C" {
  fn pthread_jit_write_protect_np(enabled: i32);
}

pub(crate) struct DataJITNote<T: FnMut(*const str) -> usize> {
  pub mem: *mut MemoryExecutable,
  #[cfg(windows)]
  pub rootaddr: usize,
  pub resolver: T,
  pub errors: Vec<Cow<'static, str>>,
  pub resolved: HashMap<Box<str>, *const Executable>,
}

impl LLVMJITLink for MemoryExecutable {
  fn write_llvm<T>(
    &mut self,
    symbolpool: &LLVMSymbolPool,
    object: &[u8],
    resolver: T,
  ) -> Result<HashMap<Box<str>, *const Executable>, Cow<'static, [std::borrow::Cow<'static, str>]>>
  where
    T: FnMut(*const str) -> usize,
  {
    let oldcursor = self.cursor;
    let mut data = DataJITNote {
      mem: self,
      #[cfg(windows)]
      rootaddr: unsafe { self.rxview.byte_add(oldcursor).addr() },
      resolver,
      errors: vec![],
      resolved: HashMap::new(),
    };

    let mut rustmem = RustMemoryInterface {
      state: &mut data as *mut _ as _,
      allocateJIT: Some(allocate_jit::<T>),
      freeJITStructure: Some(free_jit),
      getfnPtr: Some(get_fn_ptr::<T>),
      onError: Some(on_err::<T>),
      storeAddr: Some(store_ptr::<T>),
    };

    unsafe {
      let ctx_ptr = create_linkctx(&mut rustmem);

      #[cfg(target_os = "macos")]
      pthread_jit_write_protect_np(0);

      if link_consume_linkctx(
        ctx_ptr,
        symbolpool.symbpool.as_ptr(),
        object.as_ptr() as _,
        object.len(),
      ) != 0
      {
        self.cursor = oldcursor;

        #[cfg(target_os = "macos")]
        pthread_jit_write_protect_np(1);
        return Err(Cow::Borrowed(&[Cow::Borrowed(
          "Could not link context pointer",
        )]));
      }

      #[cfg(target_os = "macos")]
      pthread_jit_write_protect_np(1);
    }

    if data.errors.is_empty() {
      #[allow(unused_unsafe)]
      unsafe {
        let dst_rx = self.rxview.byte_add(oldcursor);
        let len = self.cursor - oldcursor;

        // This auto becomes a noop on x64
        crate::platform::flush_icache(dst_rx as _, len);

        self.stored.fetch_add(1, Ordering::Relaxed);
        return Ok(data.resolved);
      }
    }

    self.cursor = oldcursor;
    Err(Cow::Owned(data.errors))
  }
}

unsafe extern "C" fn store_ptr<T>(state: *mut c_void, ptr: *const i8, len: usize, store_addr: u64)
where
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

unsafe extern "C" fn on_err<T>(state: *mut c_void, ptr: *const i8)
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

unsafe extern "C" fn get_fn_ptr<T>(state: *mut c_void, ptr: *const i8, size: usize) -> usize
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

unsafe extern "C" fn free_jit(_: *mut c_void, req: AllocBlockSlices) {
  if !req.allocs.is_null() {
    unsafe {
      let slice = std::ptr::slice_from_raw_parts_mut(req.allocs, req.len);
      drop(Box::from_raw(slice));
    };
  }
}

unsafe extern "C" fn allocate_jit<T>(
  state: *mut c_void,
  req: *mut AllocRequest,
  len: usize,
) -> AllocBlockSlices
where
  T: FnMut(*const str) -> usize,
{
  unsafe {
    let state = state as *mut DataJITNote<T>;

    let mexec = &mut *(*state).mem;
    let allocation = from_raw_parts(req, len);

    let start_offset = mexec.cursor;

    let mut rw_dst = mexec.rwview.byte_add(start_offset);
    let mut rx_dst = mexec.rxview.byte_add(start_offset);

    let mut out = AllocBlockSlices {
      allocs: null_mut(),
      len: 0,
    };

    let mut size_added = 0;

    // Naively keep adding
    let allocobj = allocation
      .into_iter()
      .map(|allocation| {
        let align = rw_dst.align_offset(allocation.alignment as usize);

        if align == usize::MAX {
          return None;
        }

        size_added = (size_added as usize).checked_add(align.checked_add(allocation.size)?)?;

        let addend = align.checked_add(allocation.size)?;

        // Ensure that it doesn't overflow to hell
        rw_dst.addr().checked_add(addend)?;
        rx_dst.addr().checked_add(addend)?;

        let out = AllocBlockSlice {
          rwview: rw_dst.byte_add(align).addr(),
          rxview: rx_dst.byte_add(align).addr(),
        };

        rw_dst = rw_dst.byte_add(addend);
        rx_dst = rx_dst.byte_add(addend);

        Some(out)
      })
      .try_collect::<Box<_>>();

    if let Some(allocobj) = allocobj {
      if let Some(newcursor) = start_offset.checked_add(size_added) {
        if newcursor <= mexec.size {
          out.len = allocobj.len();
          out.allocs = Box::into_raw(allocobj) as _;
          mexec.cursor = newcursor;
        }
      }
    }

    out
  }
}
