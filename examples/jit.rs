use std::mem::transmute;

use sajit::{
  MemoryExecutable,
  relocations::{RelocKind, Relocation},
};

fn main() {
  unsafe {
    let data = include!("./bin");
    let reloc = [Relocation {
      addend: 0,
      kind: RelocKind::Abs8,
      offset: 18,
      symbol_addr: myfn as *const () as usize as _,
    }];

    println!("What's linked: {}", myfn as *const () as usize);

    let code = MemoryExecutable::new(&data, &reloc).unwrap();

    let e: extern "C" fn(i32, i32) -> i32 = transmute(code.entry_ptr());

    println!("{}", e(10, 20));
  }
}

extern "C" fn myfn(a: i64) {
  println!("Hi, {a}");
}
