use object::Architecture as Arch;

use crate::SizeAlign;

pub const fn get_stubsize(arch: Arch) -> SizeAlign {
  match arch {
    Arch::X86_64 => SizeAlign {
      size: 32,
      align: 16,
    },
    Arch::X86_64_X32 => SizeAlign { size: 8, align: 4 },
    Arch::Aarch64 | Arch::Aarch64_Ilp32 => SizeAlign { size: 20, align: 8 },
    Arch::Arm => SizeAlign { size: 16, align: 4 },
    Arch::PowerPc64 => SizeAlign {
      size: 64,
      align: 16,
    },
    Arch::Riscv64 => SizeAlign { size: 32, align: 8 },
    _ => SizeAlign {
      align: 64,
      size: 64,
    },
  }
}
