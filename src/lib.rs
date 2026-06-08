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

#[cfg(windows)]
pub mod coffr;

pub mod advanced;
pub mod platform;
pub mod relcar;

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
/// is guaranteed undefined behaviour that no CPU would like
/// and would result in memory access violation, or even worse,
/// crash with the OS.
pub struct Executable;
