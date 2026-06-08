# SaJIT

> Please note that this readme is not AI generated.
>
> The emojis are for better navigation purposes only.

**SaJIT** is an Executable Region allocator and linker written in Rust with an extended ObjectFile linker in C++20

It offers a MemoryExecutable interface with a homegrown linker in Rust. However, for object files (like what LLVM outputs) we have 2 linkers in C++ (JITLinker, RuntimeDyld) that require C++20 under the `llvm` feature.

We support the following executable api:

- MemoryExecutableApi (Rust)

<h2>🫕 OS Platform Support</h2>

<details>
  <summary>Click here to expand</summary>

| Operating System | Arch        | Status  | Notes                                                                                                      |
| ---------------- | ----------- | ------- | ---------------------------------------------------------------------------------------------------------- |
| Windows          | x86_64      | ✅ (🥇) |                                                                                                            |
|                  | x86         | ❌ (🟨) | Only COFFR support                                                                                         |
|                  | arm64       | ✅ (🟨) | RELCAR support good, no JITLink and minimal RTDyld                                                         |
| Linux            | x86_64      | ✅ (🥇) |                                                                                                            |
|                  | x86         | 🟨      | Testing infrastructure welcome                                                                             |
|                  | arm64       | ✅      |                                                                                                            |
|                  | armv7       | ✅      | Testing infrastructure welcome                                                                             |
|                  | riscv64     | ✅      |                                                                                                            |
|                  | riscv32     | 🏗️      | Testing infrastructure welcome                                                                             |
|                  | loongarch64 | 🏗️      | TODO: Build LLVM loongarch64 [llvm](https://github.com/savmlang/llvm/blob/main/.github/workflows/llvm.yml) |
|                  | powerpc64le | ✅      | Testing infrastructure welcome                                                                             |
|                  | mips64el    | 🏗️      | Testing infrastructure welcome                                                                             |
| macOS            |             |         | Gatekeeper might block JIT. Be advised                                                                     |
|                  | x86_64      | ✅      |                                                                                                            |
|                  | arm64       | ✅      |                                                                                                            |
| Android          | x86_64      | ❌      | Android has unintended friction                                                                            |
|                  | x86         | ❌      | towards memory mapped code due to                                                                          |
|                  | armv7       | ❌      | security reasons.                                                                                          |
|                  | arm64       | ❌      |                                                                                                            |
| iOS              | arm64       | ❌      | Experimental, Hacky, not worth it.                                                                         |

🥇: Maintainer Environment
✅: Supported
🟨: Tests Pending
🏗️: Hacky
❌: Unlikely to be supported

</details>

## 🔗 Linker Platform Matrix

The below table should be a good heuristic about support (JITLink and RuntimeDyld may be incorrect. Consult LLVM)

| Operating System | Arch        | [RELCAR \*](#-sajit-relcar) | [COFFR \*](#-sajit-coffr) | [JITLink \*](#-llvm-jitlink) | [RuntimeDyld \*](#-llvm-runtimedyld) |
| ---------------- | ----------- | --------------------------- | ------------------------- | ---------------------------- | ------------------------------------ |
| Windows          | x86_64      | ✅                          | ❌                        | ✅                           | ✅                                   |
|                  | x86         | 🟨                          | ✅                        | ❌                           | 🟨                                   |
|                  | arm64       | ✅                          | ❌                        | ❌                           | ✅                                   |
| Linux            | x86_64      | ✅                          | ❌                        | ✅                           | ✅                                   |
|                  | x86         | 🟨                          | ❌                        | ✅                           | ✅                                   |
|                  | arm64       | ✅                          | ❌                        | ✅                           | ✅                                   |
|                  | armv7       | 🟨                          | ❌                        | ✅                           | ✅                                   |
|                  | riscv64     | ✅                          | ❌                        | ✅                           | ✅                                   |
|                  | riscv32     | ❌                          | ❌                        | ❌                           | 🟨                                   |
|                  | loongarch64 | ❌                          | ❌                        | ❌                           | ✅                                   |
|                  | powerpc64le | ❌                          | ❌                        | ✅                           | ✅                                   |
|                  | mips64el    | ❌                          | ❌                        | ❌                           | ✅                                   |
| macOS            | x86_64      | ✅                          | ❌                        | ✅                           | ✅                                   |
|                  | arm64       | ✅                          | ❌                        | ✅                           | ✅                                   |

✅: Supported
🟨: Limbo - prefer others
❌: Not Supported

- **RELCAR** cannot process ObjectFiles

## 🚂 Relocators

Available relocators :

- SaJIT RELCAR (Rust)
- SaJIT COFFR (Rust)
- LLVM JITLink (C++)
- LLVM RuntimeDyld (C++)

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

This a relocator only for parsing PE/COFF objects and is **ONLY** intended for i386 where all the LLVM Alternatives spectacularly fail.
This is a heavily minimal COFF parser and relocator for only **i386** windows

Some parts from : [coffeldr](https://github.com/joaoviictorti/coffeeldr) are explicitly tagged with their APACHE license

### 📍 Implementations

| Name                 | Note             | Range |
| :------------------- | ---------------- | ----- |
| IMAGE_REL_I386_DIR32 | 🏗️ Tests Pending | FULL  |
| IMAGE_REL_I386_REL32 | 🏗️ Tests Pending | FULL  |

## 🔗 LLVM JITLink

We have a C++ mapping of LLVM JITLink to support advanced relocations and linking. This is exposed by the `LLVMJITLink` trait. This is the recommended linker for production projects.

**Platform Support:** [LLVM JITLink](https://llvm.org/docs/JITLink.html#jitlink-availability-and-feature-status)

## 📖 LLVM RuntimeDyld

We also offer a C++ mapping of LLVM RuntimeDyld as a relocator and linker. This is exposed by the `LLVMRTDyld` trait. This is only recommended as a fallback for `LLVMJITLink`.

Supported in both **JITLink** and **RuntimeDyld**:

| Format | Arch                                                    |
| ------ | ------------------------------------------------------- |
| COFF   | x86-64                                                  |
| ELF    | aarch32, aarch64, i386, LoongArch, PPC64, RISCV, x86-64 |
| MachO  | aarch64, x86-64                                         |

Supported only in **RuntimeDyld**:

| Format | Arch                   |
| ------ | ---------------------- |
| COFF   | aarch32, aarch64, i386 |
| ELF    | MIPS, PPC32, SPARC     |
| MachO  | aarch32, i386          |

## 🏗️ Maintainer's Choice

Since the above can be exhausting to decide. Here is a small mental model map to aid in decision making.
Maintainer [@ahqsoftwares](https://github.com/ahqsoftwares) believes the following is the optimal workflow.

```
Cranelift (X64, Arm64, Riscv64 ABSOLUTE) = RELCAR
Object File (X64 ELF/MachO, Arm64 ELF/MachO) = JITLink
Object File (X64 COFF, Arm64 COFF) = RuntimeDyld
Object File (I386 COFF) = COFFR [🫕 THE ONLY TARGET COFFR HAS]
```
