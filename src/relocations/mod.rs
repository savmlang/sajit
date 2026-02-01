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
  Abs8 = 0,
  X86CallPCRel4 = 1,
  X86PCRel4 = 2,
  // Aarch64
  Arm64Call = 3,
  // Cranelift doesn't generate these for
  // function calls
  //
  // Aarch64AdrGotPage21 = 4,
  // Aarch64Ld64GotLo12Nc = 5,
  // Aarch64AddAbsLo12Nc = 6,
}
