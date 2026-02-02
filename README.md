# SaJIT

The JIT Loader for the Sa VM Programming Language. This is built to handle allocation, library linking, relocation Just-In-Time before the code loads into R^X mode.

## Platform Support

| Operating System | Arch   | Status | Notes                                                   |
| ---------------- | ------ | ------ | ------------------------------------------------------- |
| Windows          | x86_64 | ✅     | [👇](#x86_64-supported-relocation)                      |
|                  | arm64  | ✅     | [👇](#arm64-supported-relocation) |
| Linux            | x86_64 | ✅     | [👇](#x86_64-supported-relocation)                      |
|                  | arm64  | ✅     | [👇](#arm64-supported-relocation) |
| Darwin           | x86_64 | ❌     | macOS is not on our list                                |
|                  | arm64  | ❌     | due to unintended friction from the platform            |
| Android          | x86_64 | ❌     |                                                         |
|                  | x86    | ❌     |                                                         |
|                  | armv7  | ❌     |                                                         |
|                  | arm64  | ❌     |                                                         |
| iOS              | arm64  | ❌     |                                                         |

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

## Sidenote

You do not need to perform relocation calculations. The loader resolves and patches all addresses; you only provide the absolute target address.

Please note that the supported relocation list is generated from our own tests with cranelift-codegen crate for relocations of function `call`s. The names are directly borrowed from their list and we only link a subset of them.
