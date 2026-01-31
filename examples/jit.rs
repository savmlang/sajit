use sajit::platform::win32::{FlushInstructionCache, GetCurrentProcess};

fn main() {
  unsafe {
    let proc = GetCurrentProcess();

    let data = include!("./bin");

    let pt = data.as_ptr();
    let out = FlushInstructionCache(proc, pt as _, 10);

    println!("{:?} {}", proc, out);
  }
}
