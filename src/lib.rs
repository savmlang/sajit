//! SaJIT Loader
//!
//! SaJIT is a handwritten cross-platform cross-architecture hand rolled loader
//! designed to be extremely lean and efficient with incredible relocating precision.
//!
//! It is fundamentally a slab allocator - linker combination designed for performance
//! intensive workloads
//!
//! It fundamentally deviates from the general norm into high performance computing
//! and uses file-backed pages on platforms like windows

/// The COFFR Linker
pub mod coffr;

/// Memory Executable & other advanced apis
pub mod advanced;

/// Platform specific mechanics
pub mod platform;
pub mod relcar;

/// Core types surrounding relocations
pub mod relocations;

pub use advanced::*;

/// This is a wrapper structure
///
/// This is literally meant to annotate outputs
/// that are mounted with the RX flags.
///
/// This is in all of truth, just bytes, but it is
/// bytes in read+execute mode,
///
/// feel free to `transmute` it as anything that is
/// executable.
///
/// Casting a `*const Executable` as `*mut Executable`
/// is guaranteed undefined behaviour that would result 
/// in memory access violation.
#[repr(C)]
pub struct Executable;
