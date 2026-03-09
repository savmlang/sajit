# SaJIT

The JIT Loader for the Sa VM Programming Language. This is built to handle allocation, library linking, relocation Just-In-Time before the code loads into R^X mode.

## Platform Support

| Operating System | Arch        | Status | Notes                              |
| ---------------- | ----------- | ------ | ---------------------------------- |
| Windows          | x86_64      | ✅     | [👇](#x86_64-supported-relocation) |
|                  | x86         | 🟨     | [👇](#x86-supported-relocation)    |
|                  | arm64       | ✅     | [👇](#arm64-supported-relocation)  |
| Linux            | x86_64      | ✅     | [👇](#x86_64-supported-relocation) |
|                  | x86         | 🟨     | [👇](#x86-supported-relocation)    |
|                  | arm64       | ✅     | [👇](#arm64-supported-relocation)  |
|                  | armv7       | 🟨     | [👇](#armv7-supported-relocation)  |
|                  | riscv64     | 🟨     |                                    |
|                  | riscv32     | 🟨     |                                    |
|                  | loongarch64 | 🟨     |                                    |
| Darwin           | x86_64      | 🟨     | macOS might be implemented         |
|                  | arm64       | 🟨     |                                    |
| Android          | x86_64      | ❌     | Android has unintended friction    |
|                  | x86         | ❌     | towards memory mapped code due to  |
|                  | armv7       | ❌     | security reasons                   |
|                  | arm64       | ❌     |                                    |
| iOS              | arm64       | ❌     | Experimental, Hacky, not worth it  |

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

## X86 Supported Relocation

Our JIT Loader only handles the following x86 relocations.

| Name            | Note                  |
| :-------------- | --------------------- |
| RelocKind::Abs4 | Uses absolute pointer |

## Arm64 Supported Relocation

Our JIT Loader only handles the following aarch64 relocations.

| Name                 | Note                                  |
| :------------------- | ------------------------------------- |
| RelocKind::Abs8      | Uses absolute pointer                 |
| RelocKind::Arm64Call | Uses relative pointer, range +-128MiB |

## Armv7 Supported Relocation

Our JIT Loader only handles the following armv7 relocations.

| Name            | Note                  |
| :-------------- | --------------------- |
| RelocKind::Abs4 | Uses absolute pointer |

## Sidenote

You do not need to perform relocation calculations. The loader resolves and patches all addresses; you only provide the absolute target address.

Please note that the supported relocation list is generated from our own tests with cranelift-codegen crate for relocations of function `call`s. The names are directly borrowed from their list and we only link a subset of them.
