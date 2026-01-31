use clear_cache::clear_cache;

pub fn flush_icache(base: *mut c_void, size: usize) -> bool {
  let end = unsafe { base.add(size) };
  unsafe { clear_cache(base, end) }
}
