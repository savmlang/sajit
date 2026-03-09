#[repr(C)]
pub struct Relocation {
  pub addend: i64,
  /// The address to the symbol
  pub symbol_addr: u64,
  pub offset: u32,
  pub kind: RelocKind,
}

#[repr(C)]
pub enum RelocKind {
  // X86-64
  /// This describes an absolute 64-bit call
  Abs8 = 0,

  /// This describes a signed 32-bit relative offset call (+- 2GiB)
  X86CallPCRel4 = 1,
  /// This describes a signed 32-bit relative offset call (+- 2GiB)
  X86PCRel4 = 2,
  // Aarch64
  /// This is the arm64 24-bit relative call (+-128MiB)
  Arm64Call = 3,
  // X86
  /// This describes an Intel `i686` 8-bit absolute address
  /// Since intel x86 are 32-bit, this is the only one required
  Abs4 = 4,
  // Cranelift doesn't generate these for
  // function calls
  //
  // Aarch64AdrGotPage21 = 4,
  // Aarch64Ld64GotLo12Nc = 5,
  // Aarch64AddAbsLo12Nc = 6,
}
