use std::{mem::transmute, ptr};

use memmap2::MmapOptions;

fn main() {
  unsafe {
    let mut data = include!("./bin");

    {
      let idx = data.as_mut_ptr().add(18) as *mut usize;

      ptr::write_unaligned(idx, myfn as *mut usize as usize);
    }

    let mut mmap = MmapOptions::new();

    {
      mmap.len(data.len());

      let mut anon = mmap.map_anon().unwrap();

      anon.copy_from_slice(&data);

      let f = anon.make_exec().unwrap();

      {
        let cdcl: extern "C" fn(a: i64, b: i64) -> i64 = transmute(f.as_ptr());

        println!("{}", cdcl(1, 2));
      }
    }
  }
}

extern "C" fn myfn(a: i64) {
  println!("Hi, {a}");
}
