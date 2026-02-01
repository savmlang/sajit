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
  Abs8,
  X86CallPCRel4,
  X86PCRel4,
  // Aarch64
  Arm64Call,
  Aarch64AdrGotPage21,
  Aarch64Ld64GotLo12Nc,
  Aarch64AddAbsLo12Nc,
}
