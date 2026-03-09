use std::{
  ffi::CStr,
  fs::File,
  io::{Read, Seek, SeekFrom, Write},
  os::fd::FromRawFd,
};

use libc::{MFD_CLOEXEC, memfd_create};

const GREETING: &CStr = c"Hello, world!";

fn main() {
  unsafe {
    let fd = memfd_create(GREETING.as_ptr(), MFD_CLOEXEC);

    let mut file = File::from_raw_fd(fd);

    file
      .write_all(&[0xB8, 0x2A, 0x00, 0x00, 0x00, 0xC3])
      .unwrap();
    file.flush().unwrap();

    let mut data = vec![];
    file.seek(SeekFrom::Start(0)).unwrap();
    file.read_to_end(&mut data).unwrap();

    println!("{data:#?}");
  }
}
