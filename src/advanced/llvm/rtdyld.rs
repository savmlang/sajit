use std::{collections::HashMap, ffi::c_void, slice::from_raw_parts, sync::atomic::Ordering};

use crate::{
  LLVMRTDyld, MemoryExecutable,
  llvm::DataJITNote,
  relocations::llvmreloc::{
    jitlink,
    rtdyld::{AllocBlockSlice, AllocRequest, RustRTInterface, SectionName, link_rtdyld},
  },
};

#[cfg(target_os = "macos")]
#[link(name = "pthread")]
unsafe extern "C" {
  fn pthread_jit_write_protect_np(enabled: i32);
}

impl LLVMRTDyld for MemoryExecutable {
  fn write_rtdyld<T>(
    &mut self,
    object: &[u8],
    resolver: T,
  ) -> Result<std::collections::HashMap<Box<str>, *const crate::Executable>, ()>
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

    let mut rt = RustRTInterface {
      state: &mut data as *mut _ as _,
      getfnPtr: Some(get_fn_ptr::<T>),
      resolvefnOffset: Some(push_fnptr::<T>),
      allocate: Some(allocate_jit::<T>),
    };

    let output = unsafe {
      #[cfg(target_os = "macos")]
      pthread_jit_write_protect_np(0);

      let out = link_rtdyld(&mut rt, object.as_ptr() as _, object.len());

      #[cfg(target_os = "macos")]
      pthread_jit_write_protect_np(1);

      out
    };

    if output != 0 {
      self.cursor = oldcursor;
      return Err(());
    }

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
}

unsafe extern "C" fn push_fnptr<T>(state: *mut c_void, ptr: *const i8, size: usize, offset: u64)
where
  T: FnMut(*const str) -> usize,
{
  unsafe {
    let state = &mut *(state as *mut DataJITNote<T>);

    if let Ok(symbol) = str::from_utf8(from_raw_parts(ptr as _, size)) {
      let text = *state.resolved.get(".text").unwrap();

      state
        .resolved
        .entry(Box::from(symbol))
        .or_insert_with(|| text.byte_add(offset as _));
    }
  }
}

unsafe extern "C" fn get_fn_ptr<T>(state: *mut c_void, ptr: *const i8, size: usize) -> *mut c_void
where
  T: FnMut(*const str) -> usize,
{
  unsafe {
    let output = super::get_fn_ptr::<T>(state, ptr, size);

    output as *mut () as _
  }
}

unsafe extern "C" fn allocate_jit<T>(
  state: *mut c_void,
  req: AllocRequest,
  name: SectionName,
) -> AllocBlockSlice
where
  T: FnMut(*const str) -> usize,
{
  unsafe {
    let mut req = jitlink::AllocRequest {
      size: (req).size,
      alignment: (req).alignment as _,
    };
    let output = super::allocate_jit::<T>(state, &mut req as _, 1);
    let knot1 = *output.allocs;
    super::free_jit(state, output);

    let strdata = from_raw_parts(name.ptr as *const u8, name.size);
    if let Ok(data) = str::from_utf8(strdata) {
      _ = (*(state as *mut DataJITNote<T>))
        .resolved
        .insert(Box::from(data), knot1.rxview as _);
    }

    AllocBlockSlice {
      rwview: knot1.rwview,
      rxview: knot1.rxview,
    }
  }
}
