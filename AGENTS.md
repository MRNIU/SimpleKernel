# AGENTS.md — SimpleKernel

## OVERVIEW
Interface-driven OS kernel for AI-assisted learning. C++23/C23, freestanding, no RTTI/exceptions. Three architectures: x86_64, riscv64, aarch64. Headers define contracts (Doxygen @pre/@post), AI generates .cpp implementations, tests verify compliance.

## STRUCTURE
```
src/include/          # Public interface headers — READ FIRST
src/arch/             # Per-architecture code (see src/arch/AGENTS.md)
src/device/           # Device framework, drivers (see src/device/AGENTS.md)
src/task/             # Schedulers, TCB, sync (see src/task/AGENTS.md)
src/filesystem/       # VFS, RamFS, FatFS (see src/filesystem/AGENTS.md)
src/libc/             # Kernel C stdlib (sk_ prefix headers)
src/libcxx/           # Kernel C++ runtime (sk_ prefix headers)
tests/                # Unit/integration/system tests (see tests/AGENTS.md)
cmake/                # Toolchain files, build helpers
3rd/                  # Git submodules (opensbi, u-boot, googletest, bmalloc, ...)
```

## WHERE TO LOOK
- **Implementing a module** → Read interface header in `src/include/` first, then arch-specific in `src/arch/{arch}/`
- **Adding a driver** → `src/device/include/driver/` for examples, `driver_registry.hpp` for registration
- **Adding a scheduler** → `src/task/include/scheduler_base.hpp` for base class
- **Boot flow** → `src/main.cpp`: _start → ArchInit → MemoryInit → InterruptInit → DeviceInit → FileSystemInit → Schedule()
- **Error handling** → `Expected<T>` (std::expected alias) in `src/include/kernel_error.h`
- **Logging** → `klog::Debug/Info/Warn/Err()` or `klog::info <<` stream API

## CODE MAP
| Interface | Purpose | Implementation |
|-----------|---------|----------------|
| `src/arch/arch.h` | Arch-agnostic entry points | `src/arch/{arch}/*.cpp` |
| `src/include/interrupt_base.h` | Interrupt subsystem ABC | `src/arch/{arch}/interrupt.cpp` |
| `src/include/virtual_memory.hpp` | Virtual memory mgmt | `src/virtual_memory.cpp` |
| `src/include/kernel_fdt.hpp` | Device tree parser | header-only (utility exception) |
| `src/include/kernel_elf.hpp` | ELF parser | header-only (utility exception) |
| `src/include/spinlock.hpp` | Spinlock | header-only (__always_inline) |
| `src/include/mutex.hpp` | Mutex | `src/task/mutex.cpp` |
| `src/device/include/*.hpp` | Device framework | header-only |
| `src/task/include/*.hpp` | Task/scheduler interfaces | `src/task/*.cpp` |

## CONVENTIONS
- **Style**: Google C++ via `.clang-format`, enforced by pre-commit
- **Naming**: files=`snake_case`, classes=`PascalCase`, vars=`snake_case`, members=`snake_case_` (trailing _), constants=`kCamelCase`, macros=`SCREAMING_SNAKE`
- **Headers**: `#ifndef SIMPLEKERNEL_SRC_INCLUDE_FILENAME_HPP_` guard format
- **Copyright**: `/** @copyright Copyright The SimpleKernel Contributors */`
- **Includes**: system → third-party → project. Use `kstd_cstdio`/`kstd_vector`, never std dynamic alloc
- **Returns**: trailing return type `auto Func() -> RetType`
- **CMake**: UPPERCASE commands/keywords, 4-space indent, 80-char lines, space before `(`

## ANTI-PATTERNS
- **NO** exceptions, RTTI, `dynamic_cast`, `typeid`
- **NO** heap allocation before memory subsystem init — use `src/libc/` and `src/libcxx/`
- **NO** standard library dynamic containers in freestanding — use `sk_` prefixed versions
- **NO** implementation in interface headers (exception: `__always_inline` perf-critical, utility parsers)
- **NO** modifying interface .h/.hpp files to add implementation
- **NO** `as any`/type suppression equivalents

## UNIQUE STYLES
- `Expected<T>` for all error returns (no exceptions)
- `etl::singleton<T>` with named aliases in `kernel.h` (e.g. `TaskManagerSingleton::instance()`, `DeviceManagerSingleton::create()`)
- `LockGuard<SpinLock>` RAII locking
- `__builtin_unreachable()` for dead code paths
- `[[nodiscard]]`, `[[maybe_unused]]` attributes used extensively
- Doxygen `@pre`/`@post`/`@brief` on every interface method

## COMMANDS
```bash
git submodule update --init --recursive   # First clone setup
cmake --preset build_{riscv64|aarch64|x86_64}
cd build_{arch} && make SimpleKernel       # Build kernel (NOT 'make kernel')
make run                                   # Run in QEMU
make debug                                 # GDB on localhost:1234
cmake --preset build_{arch} && cd build_{arch} && make unit-test  # Host-only tests
make coverage                              # Tests + coverage report
pre-commit run --all-files                 # Format check
```

## NOTES
- Interface-driven: headers are contracts, .cpp files are implementations AI generates
- Boot chains differ: x86_64 (U-Boot), riscv64 (U-Boot SPL→OpenSBI→U-Boot), aarch64 (U-Boot→ATF→OP-TEE)
- aarch64 needs two serial terminal tasks (::54320, ::54321) before `make run`
- Unit tests only run on host arch (`build_{arch}` on {arch} host)
- Git commits: `<type>(<scope>): <subject>` with `--signoff`
- Debug artifacts in `build_{arch}/bin/` (objdump, nm, map, dts, QEMU logs)
