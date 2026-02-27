# AGENTS.md — src/filesystem/

## OVERVIEW
Three-layer filesystem: VFS abstraction (Inode/Dentry/File/Superblock), RamFS (in-memory, static pool allocation), FatFS (3rd-party FAT wrapper over VirtIO block device). FileDescriptor table per-task.

## STRUCTURE
```
include/
  file_descriptor.hpp     # FileDescriptor — per-task FD table
filesystem.cpp            # FileSystemInit() — mounts root RamFS, registers FatFS
file_descriptor.cpp       # FD operations implementation
vfs/
  include/
    dentry.hpp            # Dentry — directory entry cache, name→inode mapping
    file.hpp              # File — open file state, position, ops
    inode.hpp             # Inode — filesystem object metadata + operations table
    mount.hpp             # Mount point management
    superblock.hpp        # Superblock — per-filesystem instance state
    vfs.hpp               # VFS public API — open, read, write, close, mkdir, etc.
  vfs_internal.hpp        # VFS internal helpers (not exported)
  vfs.cpp                 # VFS core — Dentry cache, superblock management
  open.cpp close.cpp read.cpp write.cpp  # Per-syscall VFS operations
  lookup.cpp mkdir.cpp rmdir.cpp unlink.cpp seek.cpp readdir.cpp mount.cpp
ramfs/
  include/
    ramfs.hpp             # RamFS — register_filesystem, Inode/Dentry ops
  ramfs.cpp               # RamFS implementation — static pool, 605 lines
fatfs/
  include/
    fatfs.hpp             # FatFS kernel wrapper
  fatfs.cpp               # FatFS → VFS bridge
  diskio.cpp              # FatFS disk I/O — routes to VirtIO block device
```

## WHERE TO LOOK
- **Adding a filesystem** → Implement Superblock/Inode/Dentry/File ops, register via `register_filesystem()`
- **VFS operations** → One .cpp per syscall in `vfs/` (open.cpp, read.cpp, etc.)
- **Block device I/O** → `fatfs/diskio.cpp` → `block_device_provider.hpp` in device/
- **Mount flow** → `filesystem.cpp` → mounts "/" as RamFS at boot

## CONVENTIONS
- VFS follows Linux-style layering: Superblock → Inode → Dentry → File
- One .cpp per VFS operation (same pattern as task/ syscalls)
- RamFS uses static pool allocation (no heap) — fixed-size inode/dentry arrays
- FatFS wraps 3rd-party `ff.h` library — `diskio.cpp` is the HAL adaptation layer
- `vfs_internal.hpp` is VFS-private — not included outside `vfs/`

## ANTI-PATTERNS
- **DO NOT** include `vfs_internal.hpp` outside `vfs/` directory
- **DO NOT** allocate heap in RamFS — uses static pools pre-sized at compile time
- **DO NOT** call filesystem operations before `FileSystemInit()` in boot sequence
- **DO NOT** bypass VFS layer to access RamFS/FatFS directly — use `vfs.hpp` API
