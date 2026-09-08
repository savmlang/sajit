# SaJIT

> Please note that this readme is not AI generated.
>
> The emojis are for better navigation purposes only.

**SaJIT** is an Executable Region allocator and linker written in Rust with an extended ObjectFile linker in C++20

It offers a MemoryExecutable interface with a homegrown linker in Rust. However, for object files (like what LLVM outputs) we have a linker in C++ (JITLink) that requires C++20 and the `llvm` feature.

We support the following executable api:

- MemoryExecutableApi (Rust)

<h2>🫕 OS Platform Support</h2>

<details>
  <summary>Click here to expand</summary>

| Operating System | Arch        | Status   | Notes                                  |
| ---------------- | ----------- | -------- | -------------------------------------- |
| Windows          | x86_64      | ✅ (🥇)  | COFFR & JITLink                        |
|                  | x86         | 🟨       | COFFR Support                          |
|                  | arm64       | ✅       | COFFR Support                          |
| Linux            | x86_64      | ✅ (🥇)  |                                        |
|                  | x86         | 🟨       |                                        |
|                  | arm64       | ✅       |                                        |
|                  | armv7       | 🟨       | QEMU Testing successful                |
|                  | riscv64     | 🟨       | QEMU Testing successful                |
|                  | powerpc64le | 🟨       | QEMU Testing successful                |
| macOS            |             |          | Gatekeeper might block JIT. Be advised |
|                  | x86_64      | ✅ (🏗️!) | Testing no longer done                 |
|                  | arm64       | ✅       |                                        |
| Android          | x86_64      | ❌       | Android has unintended friction        |
|                  | x86         | ❌       | towards memory mapped code due to      |
|                  | armv7       | ❌       | security reasons.                      |
|                  | arm64       | ❌       |                                        |
| iOS              | arm64       | ❌       | Experimental, Hacky, not worth it.     |

🥇: Maintainer Environment
✅: Supported
🟨: Hardware Tests Pending
🏗️: Dropped
❌: Unlikely to be supported

</details>

## 🔗 Linker Platform Matrix

The below table should be a good heuristic about support (JITLink matrix may be incorrect. Consult LLVM)

| Operating System | Arch         | [RELCAR \*](#-sajit-relcar) | [COFFR \*](#-sajit-coffr) | [JITLink \*](#-llvm-jitlink) |
| ---------------- | ------------ | --------------------------- | ------------------------- | ---------------------------- |
| Windows          | x86_64       | 🟦                          | ✅                        | 🟨                           |
|                  | x86          | 🟦                          | ✅                        | ❌                           |
|                  | arm64        | 🟦                          | ✅                        | ❌                           |
| Linux            | x86_64       | 🟦                          | ❌                        | ✅                           |
|                  | x86          | 🟦                          | ❌                        | 🟨                           |
|                  | arm64        | 🟦                          | ❌                        | ✅                           |
|                  | armv7        | 🟦                          | ❌                        | 🟨                           |
|                  | riscv64      | 🟦                          | ❌                        | ✅                           |
|                  | powerpc64le  | 🟦                          | ❌                        | ✅                           |
| macOS            | x86_64 (🏗️!) | 🟦                          | ❌                        | ✅                           |
|                  | arm64        | 🟦                          | ❌                        | ✅                           |

✅: Supported
🟨: Average Support
🟦: Elementary Support
🏗️: Unlisting soon
❌: Not Supported

- **RELCAR** cannot process ObjectFiles

## 🚂 Relocators

Available relocators :

- SaJIT RELCAR (Rust)
- SaJIT COFFR (Rust)
- LLVM JITLink (C++)

## 🔨 SaJIT RELCAR

SaJIT **Rust Efficient reLocator for Configurable Address Relocation** is a simple and single-pass memory relocator that takes simple relocation types (eg. cranelift-codegen) and relocates it.

SaJIT _RELCAR_ is an extensible relocator and the default **BasicRelocator** should handle most of simple JIT use case.

### 📍 BasicRelocator

| Name                                                | Architecture           | Note                                  | Range   |
| :-------------------------------------------------- | :--------------------- | ------------------------------------- | ------- |
| RelocKind::Abs8                                     | x86_64, arm64, riscv64 | `*ptr = addr`                         | FULL    |
| RelocKind::Abs4                                     | i686, armv7            | `*ptr = addr`                         | FULL    |
| RelocKind::X86CallPCRel4 or RelocKind::X86PCRel4    | x86_64                 | Relative to `PC+4`                    | ±2GiB   |
| RelocKind::X86CallPCRelOrPCRelProvidedRelativeBytes | x86_64                 | Directly patches treating addr as i32 | ±2GiB   |
| RelocKind::Arm64Call                                | arm64                  | B/BL 26-bit immediate only            | ±128MiB |
| RelocKind::Arm64CallProvidedRelativeBytes           | arm64                  | Directly patches treating addr as i24 | ±128MiB |

### 🪟 SaJIT COFFR

This is a relocator for patching PE/COFF objects written following the spectacularly bad support for LLVM Linkers (both RuntimeDyld and JITLink) on Windows/PE/COFF

### 📍 Implementations

#### I386

1. IMAGE_REL_I386_ABSOLUTE
2. IMAGE_REL_I386_DIR32
3. IMAGE_REL_I386_DIR32NB
4. IMAGE_REL_I386_REL32
5. IMAGE_REL_I386_SECREL
6. IMAGE_REL_I386_SECTION

#### X64

1. IMAGE_REL_AMD64_ABSOLUTE
2. IMAGE_REL_AMD64_ADDR32
3. IMAGE_REL_AMD64_ADDR32NB
4. IMAGE_REL_AMD64_ADDR64
5. IMAGE_REL_AMD64_REL32 (trampolines supported)
6. IMAGE_REL_AMD64_REL32_1
7. IMAGE_REL_AMD64_REL32_2
8. IMAGE_REL_AMD64_REL32_3
9. IMAGE_REL_AMD64_REL32_4
10. IMAGE_REL_AMD64_REL32_5
11. IMAGE_REL_AMD64_SECREL
12. IMAGE_REL_AMD64_SECTION

#### ARM64

1. IMAGE_REL_ARM64_ABSOLUTE
2. IMAGE_REL_ARM64_ADDR32
3. IMAGE_REL_ARM64_ADDR32NB
4. IMAGE_REL_ARM64_ADDR64
5. IMAGE_REL_ARM64_BRANCH14
6. IMAGE_REL_ARM64_BRANCH19
7. IMAGE_REL_ARM64_BRANCH26 (trampolines supported)
8. IMAGE_REL_ARM64_PAGEBASE_REL21
9. IMAGE_REL_ARM64_PAGEOFFSET_12A
10. IMAGE_REL_ARM64_PAGEOFFSET_12L
11. IMAGE_REL_ARM64_REL21
12. IMAGE_REL_ARM64_REL32
13. IMAGE_REL_ARM64_SECTION

> **Note:** Relocations exceeding hardware limits (limited to x64 `REL32` outside ±2GiB, arm64 `BRANCH26` outside ±128MiB) automatically synthesize deduplicated stubs inside the slab-local trampoline pool.

## 🔗 LLVM JITLink

We have a C++ mapping of LLVM JITLink to support advanced relocations and linking. This is exposed by the `LLVMJITLink` trait. This is the recommended linker for production projects.

> [!WARNING]
> **Integration Stability:** The current C++/Rust bridging layer for JITLink is under active refinement. Memory manager callbacks and asynchronous symbol resolution across the FFI boundary are somewhat brittle. A deeper, idiomatic integration with SaJIT's native slab allocator is planned as an incremental update. Issue reports and edge-case reproductions are welcome!

**Platform Support:** [LLVM JITLink](https://llvm.org/docs/JITLink.html#jitlink-availability-and-feature-status)

## 📖 LLVM RuntimeDyld

> RuntimeDyld was previously shipped as a SaJIT Linker. Following the development of **COFFR**, this has been removed to streamline development and reduce the vulnerability surface across C++/Rust interop.

<details>
  <summary>Why removal?</summary>

The primary reasons are as follows:

1. **Fatal Aborts:** The linker had a tendency to call unrecoverable fatal error/abort branches (`report_fatal_error`) rather than returning catchable errors.
2. **Ungraceful OOM Handling:** The linker did not handle allocation exhaustion gracefully, complicating memory bounds enforcement.
3. **Unpredictable Memory Footprint:** No reliable way existed to pre-calculate or bound the maximum memory footprint needed for a given module.
4. **Fragile Windows/COFF Support:** COFF support in RuntimeDyld was historically neglected and brittle; COFFR now provides native, deterministic handling.
5. **Non-standard Trampolines:** Out-of-range branch stubbing was non-standard and prone to relocation reach failures on 64-bit platforms.
6. **Modern Upstream Focus:** Upstream LLVM has deprecated RuntimeDyld in favor of JITLink, which already serves as SaVM’s primary object-linking engine on non-Windows platforms.
</details>

## 🏗️ Maintainer's Choice

Since the above can be exhausting to decide. Here is a small mental model map to aid in decision making.
Maintainer [@ahqsoftwares](https://github.com/ahqsoftwares) believes the following is the optimal workflow.

```
Cranelift (X64, Arm64, Riscv64 ABSOLUTE) = RELCAR
Object File (ELF, MachO) = JITLink
Object File (COFF) = COFFR
```
