# SaJIT

The JIT Loader for the Sa VM Programming Language. This is built to handle allocation, library linking, relocation Just-In-Time before the code loads into R^X mode.

## Platform Support

| Operating System | Arch   | Status | Notes                                              |
| ---------------- | ------ | ------ | -------------------------------------------------- |
| Windows          | x86_64 | 🟨     | Only absolute linkage is supported                 |
|                  | arm64  | ⏲️     | Will be considered later                           |
| Linux            | x86_64 | 🟨     | Only absolute linkage is supported                 |
|                  | arm64  | ⏲️     | Will be considered later                           |
| Darwin           | x86_64 | ❌     | Intel macOS is obsolete                            |
|                  | arm64  | ❌     | This is not intended for the near (or, far) future |
| Android          | x86_64 | ❌     |                                                    |
|                  | x86    | ❌     |                                                    |
|                  | armv7  | ❌     |                                                    |
|                  | arm64  | ❌     |                                                    |
| iOS              | arm64  | ❌     |                                                    |

## Support for Windows/Linux arm64

Support for windows and linux arm64 is being considered and platforms with ⏲️ will be implemented in the near future.
