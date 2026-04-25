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
  ///
  /// `symbol_addr` still corresponds to absolute address - we calculate
  /// displacement
  X86CallPCRel4 = 1,
  /// This describes a signed 32-bit relative offset call (+- 2GiB)
  ///
  /// `symbol_addr` still corresponds to absolute address - we calculate
  /// displacement
  X86PCRel4 = 2,

  /// This describes a signed 32-bit relative offset call (+- 2GiB)
  ///
  /// `symbol_addr` corresponds to **relative displacement**
  /// and is treated as i64 not u64
  ///
  /// `addend` is ignored
  X86CallPCRelOrPCRelProvidedRelativeBytes = 3,

  // Aarch64
  /// This is the arm64 24-bit relative call (+-128MiB)
  Arm64Call = 4,

  // Aarch64
  /// This is the arm64 24-bit relative call (+-128MiB)
  ///
  /// Unlike `Arm64Call`, `symbol_addr` corresponds to **relative byte
  /// displacement**
  ///
  /// `addend` is ignored
  Arm64CallProvidedRelativeBytes = 5,

  // X86
  /// This describes an Intel `i686` 8-bit absolute address
  /// Since intel x86 are 32-bit, this is the only one required
  Abs4 = 6,
}
