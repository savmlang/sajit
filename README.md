# SaJIT

SaJIT is a handwritten cross-platform cross-architecture hand rolled loader designed to be extremely lean and efficient with incredible relocating precision.

It is fundamentally a slab allocator - linker combination designed for performance intensive workloads.

and it uses RX, RW paging system (macOS is an exception)

It fundamentally deviates from the general norm into high performance computing and uses file-backed pages on platforms like windows

## Platform Support

| Operating System | Arch        | Status | Notes                                           |
| ---------------- | ----------- | ------ | ----------------------------------------------- |
| Windows          | x86_64      | ✅     | [👇](#x86_64-supported-relocation)              |
|                  | x86         | 🟨     | [👇](#armv7-x86-riscv32-supported-relocation)   |
|                  | arm64       | ✅     | [👇](#arm64-supported-relocation)               |
| Linux            | x86_64      | ✅     | [👇](#x86_64-supported-relocation)              |
|                  | x86         | 🟨     | [👇](#armv7-x86-riscv32-supported-relocation)   |
|                  | arm64       | ✅     | [👇](#arm64-supported-relocation)               |
|                  | armv7       | 🟨     | [👇](#armv7-x86-riscv32-supported-relocation)   |
|                  | riscv64     | 🟨     | [👇](#riscv64-loongarch64-supported-relocation) |
|                  | riscv32     | 🟨     | [👇](#armv7-x86-riscv32-supported-relocation)   |
|                  | loongarch64 | 🟨     | [👇](#riscv64-loongarch64-supported-relocation) |
| macOS            |             |        | Gatekeeper might block JIT. Be advised          |
|                  | x86_64      | ✅     | [👇](#x86_64-supported-relocation)              |
|                  | arm64       | ✅     | [👇](#arm64-supported-relocation)               |
| Android          | x86_64      | ❌     | Android has unintended friction                 |
|                  | x86         | ❌     | towards memory mapped code due to               |
|                  | armv7       | ❌     | security reasons.                               |
|                  | arm64       | ❌     |                                                 |
| iOS              | arm64       | ❌     | Experimental, Hacky, not worth it.              |

✅: Guaranteed support
🟨: Tests Pending
❌: Unlikely to be supported

## X86_64 Supported Relocation

Our JIT Loader only handles the following x86_64 relocations.

| Name                     | Note                                  |
| :----------------------- | ------------------------------------- |
| RelocKind::Abs8          | Uses absolute pointer                 |
| RelocKind::X86CallPCRel4 | Uses relative pointer, range +-2.1GiB |
| RelocKind::X86PCRel4     | Uses relative pointer, range +-2.1GiB |

## Arm64 Supported Relocation

Our JIT Loader only handles the following aarch64 relocations.

| Name                 | Note                                  |
| :------------------- | ------------------------------------- |
| RelocKind::Abs8      | Uses absolute pointer                 |
| RelocKind::Arm64Call | Uses relative pointer, range +-128MiB |

# Tier 2 Targets

## Armv7, X86, Riscv32 Supported Relocation

Our JIT Loader only handles the following 32-bit relocations.

| Name            | Note                  |
| :-------------- | --------------------- |
| RelocKind::Abs4 | Uses absolute pointer |

## Riscv64, Loongarch64 Supported Relocation

Our JIT Loader only handles the following 64-bit relocations.

| Name            | Note                  |
| :-------------- | --------------------- |
| RelocKind::Abs8 | Uses absolute pointer |

## Sidenote

You do not need to perform relocation calculations. The loader resolves and patches all addresses; you only provide the absolute target address.

Please note that the supported relocation list is generated from our own tests with cranelift-codegen crate for relocations of function `call`s. The names are directly borrowed from their list and we only link a subset of them.
