# Filesystem System Tests Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `ramfs_system_test` and `fatfs_system_test` to `tests/system_test/`, exercising the full VFS high-level API in QEMU.

**Architecture:** Both tests use only `vfs::` free functions (`Init`, `Open`, `Read`, `Write`, `Seek`, `MkDir`, `RmDir`, `Unlink`, `ReadDir`) plus `vfs::GetMountTable().Mount(...)`. ramfs mounts at `/`. fatfs mounts at `/mnt/fat` on the virtio-blk block device already wired up by `2026-02-27-virtio-fatfs-plan.md`. Tests follow the same pattern as `virtual_memory_test.cpp`: `auto xxx_test() -> bool` returning `false` on the first failing `EXPECT_*` macro.

**Tech Stack:** C++23 · `vfs.hpp` · `mount.hpp` · `ramfs.hpp` · `fatfs.hpp` · `block_device_provider.hpp` · `system_test.h` macros · `sk_printf`

**Prerequisite:** `2026-02-27-virtio-fatfs-plan.md` must be executed first — it wires the virtio-blk adapter (`GetVirtioBlkBlockDevice()`) and formats `rootfs.img` as FAT32. If it has not been done, the fatfs test will be skipped gracefully.

---

## Reference: Key APIs

```cpp
// VFS init + mount
vfs::Init()                                        -> Expected<void>
vfs::GetMountTable().Mount(path, &fs, device)      -> Expected<void>

// File operations (all return Expected<T>)
vfs::Open(path, flags)                             -> Expected<vfs::File*>
vfs::Close(file)                                   -> Expected<void>
vfs::Read(file, buf, count)                        -> Expected<size_t>
vfs::Write(file, buf, count)                       -> Expected<size_t>
vfs::Seek(file, offset, whence)                    -> Expected<uint64_t>

// Directory operations
vfs::MkDir(path)                                   -> Expected<void>
vfs::RmDir(path)                                   -> Expected<void>
vfs::Unlink(path)                                  -> Expected<void>
vfs::ReadDir(dir_file, dirent_array, count)        -> Expected<size_t>

// Open flags (vfs_types.hpp)
vfs::kOReadOnly   vfs::kOWriteOnly   vfs::kOReadWrite   vfs::kOCreate

// Seek whence
vfs::SeekWhence::kSet   vfs::SeekWhence::kCur   vfs::SeekWhence::kEnd

// DirEntry struct
struct DirEntry { uint64_t ino; uint8_t type; char name[256]; };

// Block device (for fatfs)
auto GetVirtioBlkBlockDevice() -> vfs::BlockDevice*;  // from block_device_provider.hpp
```

---

## Task 1: Create `tests/system_test/ramfs_system_test.cpp`

**Files:**
- Create: `tests/system_test/ramfs_system_test.cpp`

### Step 1: Write the file

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "ramfs.hpp"
#include "vfs.hpp"

#include <cstddef>
#include <cstdint>

#include "mount.hpp"
#include "sk_cstdio"
#include "sk_cstring"
#include "system_test.h"

auto ramfs_system_test() -> bool {
  sk_printf("ramfs_system_test: start\n");

  // T1: Init VFS
  auto init_result = vfs::Init();
  EXPECT_TRUE(init_result.has_value(), "ramfs_system_test: vfs init failed");
  sk_printf("ramfs_system_test: vfs init ok\n");

  // T1: Mount ramfs at "/"
  static ramfs::RamFs ramfs_instance;
  auto mount_result =
      vfs::GetMountTable().Mount("/", &ramfs_instance, nullptr);
  EXPECT_TRUE(mount_result.has_value(),
              "ramfs_system_test: ramfs mount at / failed");
  sk_printf("ramfs_system_test: ramfs mounted at /\n");

  // T2: Create file, write, read back
  {
    auto file_result = vfs::Open("/hello.txt", vfs::kOCreate | vfs::kOReadWrite);
    EXPECT_TRUE(file_result.has_value(),
                "ramfs_system_test: open /hello.txt failed");
    vfs::File* file = file_result.value();

    const char kMsg[] = "Hello, ramfs!";
    auto write_result = vfs::Write(file, kMsg, sizeof(kMsg) - 1);
    EXPECT_TRUE(write_result.has_value(),
                "ramfs_system_test: write failed");
    EXPECT_EQ(write_result.value(), sizeof(kMsg) - 1,
              "ramfs_system_test: write byte count mismatch");
    sk_printf("ramfs_system_test: wrote %zu bytes\n", write_result.value());

    // Seek back to start
    auto seek_result = vfs::Seek(file, 0, vfs::SeekWhence::kSet);
    EXPECT_TRUE(seek_result.has_value(),
                "ramfs_system_test: seek to start failed");
    EXPECT_EQ(seek_result.value(), static_cast<uint64_t>(0),
              "ramfs_system_test: seek position mismatch");

    // Read back
    char buf[64] = {};
    auto read_result = vfs::Read(file, buf, sizeof(buf) - 1);
    EXPECT_TRUE(read_result.has_value(),
                "ramfs_system_test: read failed");
    EXPECT_EQ(read_result.value(), sizeof(kMsg) - 1,
              "ramfs_system_test: read byte count mismatch");
    EXPECT_EQ(std::memcmp(buf, kMsg, sizeof(kMsg) - 1), 0,
              "ramfs_system_test: read content mismatch");
    sk_printf("ramfs_system_test: read back: %s\n", buf);

    vfs::Close(file);
  }

  // T3: Seek to middle, partial read
  {
    auto file_result = vfs::Open("/hello.txt", vfs::kOReadOnly);
    EXPECT_TRUE(file_result.has_value(),
                "ramfs_system_test: re-open for seek test failed");
    vfs::File* file = file_result.value();

    // Seek to offset 7
    auto seek_result = vfs::Seek(file, 7, vfs::SeekWhence::kSet);
    EXPECT_TRUE(seek_result.has_value(),
                "ramfs_system_test: seek to offset 7 failed");
    EXPECT_EQ(seek_result.value(), static_cast<uint64_t>(7),
              "ramfs_system_test: seek offset 7 mismatch");

    char buf[32] = {};
    auto read_result = vfs::Read(file, buf, 5);
    EXPECT_TRUE(read_result.has_value(),
                "ramfs_system_test: partial read failed");
    // "Hello, ramfs!" -> offset 7 = "ramfs"
    EXPECT_EQ(read_result.value(), static_cast<size_t>(5),
              "ramfs_system_test: partial read count mismatch");
    EXPECT_EQ(std::memcmp(buf, "ramfs", 5), 0,
              "ramfs_system_test: partial read content mismatch");
    sk_printf("ramfs_system_test: partial read from offset 7: %.5s\n", buf);

    vfs::Close(file);
  }

  // T4: MkDir + ReadDir
  {
    auto mkdir_result = vfs::MkDir("/testdir");
    EXPECT_TRUE(mkdir_result.has_value(),
                "ramfs_system_test: mkdir /testdir failed");
    sk_printf("ramfs_system_test: mkdir /testdir ok\n");

    // Create a file inside
    auto inner = vfs::Open("/testdir/inner.txt", vfs::kOCreate | vfs::kOWriteOnly);
    EXPECT_TRUE(inner.has_value(),
                "ramfs_system_test: open /testdir/inner.txt failed");
    vfs::Close(inner.value());

    // ReadDir on /testdir
    auto dir_file_result = vfs::Open("/testdir", vfs::kOReadOnly | vfs::kODirectory);
    EXPECT_TRUE(dir_file_result.has_value(),
                "ramfs_system_test: open /testdir as dir failed");
    vfs::File* dir_file = dir_file_result.value();

    vfs::DirEntry entries[8] = {};
    auto readdir_result = vfs::ReadDir(dir_file, entries, 8);
    EXPECT_TRUE(readdir_result.has_value(),
                "ramfs_system_test: readdir failed");
    // Expect at least "." + ".." + "inner.txt" = 3 entries
    EXPECT_GT(readdir_result.value(), static_cast<size_t>(2),
              "ramfs_system_test: readdir should return > 2 entries");
    sk_printf("ramfs_system_test: readdir returned %zu entries\n",
              readdir_result.value());

    vfs::Close(dir_file);
  }

  // T5: Unlink a file, confirm it can't be re-opened without kOCreate
  {
    auto unlink_result = vfs::Unlink("/hello.txt");
    EXPECT_TRUE(unlink_result.has_value(),
                "ramfs_system_test: unlink /hello.txt failed");
    sk_printf("ramfs_system_test: unlink /hello.txt ok\n");

    auto reopen = vfs::Open("/hello.txt", vfs::kOReadOnly);
    EXPECT_FALSE(reopen.has_value(),
                 "ramfs_system_test: /hello.txt should be gone after unlink");
    sk_printf("ramfs_system_test: confirmed /hello.txt no longer exists\n");
  }

  // T6: RmDir
  {
    // Remove inner file first
    auto unlink_result = vfs::Unlink("/testdir/inner.txt");
    EXPECT_TRUE(unlink_result.has_value(),
                "ramfs_system_test: unlink /testdir/inner.txt failed");

    auto rmdir_result = vfs::RmDir("/testdir");
    EXPECT_TRUE(rmdir_result.has_value(),
                "ramfs_system_test: rmdir /testdir failed");
    sk_printf("ramfs_system_test: rmdir /testdir ok\n");
  }

  // T7: Two independent files do not share data
  {
    auto f1 = vfs::Open("/fileA.txt", vfs::kOCreate | vfs::kOReadWrite);
    auto f2 = vfs::Open("/fileB.txt", vfs::kOCreate | vfs::kOReadWrite);
    EXPECT_TRUE(f1.has_value(), "ramfs_system_test: open fileA failed");
    EXPECT_TRUE(f2.has_value(), "ramfs_system_test: open fileB failed");

    const char kDataA[] = "AAAA";
    const char kDataB[] = "BBBB";
    vfs::Write(f1.value(), kDataA, 4);
    vfs::Write(f2.value(), kDataB, 4);

    vfs::Seek(f1.value(), 0, vfs::SeekWhence::kSet);
    vfs::Seek(f2.value(), 0, vfs::SeekWhence::kSet);

    char buf1[8] = {};
    char buf2[8] = {};
    vfs::Read(f1.value(), buf1, 4);
    vfs::Read(f2.value(), buf2, 4);

    EXPECT_EQ(std::memcmp(buf1, kDataA, 4), 0,
              "ramfs_system_test: fileA data corrupted by fileB");
    EXPECT_EQ(std::memcmp(buf2, kDataB, 4), 0,
              "ramfs_system_test: fileB data corrupted by fileA");
    sk_printf("ramfs_system_test: two files are independent\n");

    vfs::Close(f1.value());
    vfs::Close(f2.value());
    vfs::Unlink("/fileA.txt");
    vfs::Unlink("/fileB.txt");
  }

  sk_printf("ramfs_system_test: all tests passed\n");
  return true;
}
```

### Step 2: Check include paths compile

After wiring in CMakeLists.txt (Task 4), build and look for errors related to this file.

---

## Task 2: Create `tests/system_test/fatfs_system_test.cpp`

**Files:**
- Create: `tests/system_test/fatfs_system_test.cpp`

**Dependency:** `GetVirtioBlkBlockDevice()` from `block_device_provider.hpp` must exist (from `2026-02-27-virtio-fatfs-plan.md`). If it does not, the test early-returns `true` with a warning rather than failing hard, since the virtio-blk plan may not be run yet.

### Step 1: Write the file

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "fatfs.hpp"
#include "vfs.hpp"

#include <cstddef>
#include <cstdint>

#include "block_device_provider.hpp"
#include "mount.hpp"
#include "sk_cstdio"
#include "sk_cstring"
#include "system_test.h"

auto fatfs_system_test() -> bool {
  sk_printf("fatfs_system_test: start\n");

  // T1: Get virtio-blk device
  vfs::BlockDevice* blk = GetVirtioBlkBlockDevice();
  if (blk == nullptr) {
    sk_printf("fatfs_system_test: SKIP — no virtio-blk device available\n");
    return true;  // Graceful skip, not a failure
  }
  sk_printf("fatfs_system_test: virtio-blk device: %s\n", blk->GetName());
  EXPECT_GT(blk->GetSectorCount(), static_cast<uint64_t>(0),
            "fatfs_system_test: virtio-blk has zero sectors");

  // T2: Mount FatFS at /mnt/fat
  // vfs::Init() and ramfs mount at "/" should already have been done by
  // ramfs_system_test, but call Init() again (it's idempotent).
  auto init_result = vfs::Init();
  EXPECT_TRUE(init_result.has_value(), "fatfs_system_test: vfs init failed");

  // Create /mnt and /mnt/fat directories in the VFS tree (in ramfs at /)
  // before mounting. MkDir is idempotent-ish — ignore errors if exists.
  vfs::MkDir("/mnt");
  vfs::MkDir("/mnt/fat");

  static fatfs::FatFsFileSystem fat_fs(0);
  auto fat_mount = fat_fs.Mount(blk);
  EXPECT_TRUE(fat_mount.has_value(), "fatfs_system_test: FatFsFileSystem::Mount failed");
  sk_printf("fatfs_system_test: FatFsFileSystem::Mount ok\n");

  auto vfs_mount = vfs::GetMountTable().Mount("/mnt/fat", &fat_fs, blk);
  EXPECT_TRUE(vfs_mount.has_value(),
              "fatfs_system_test: vfs mount at /mnt/fat failed");
  sk_printf("fatfs_system_test: vfs mount at /mnt/fat ok\n");

  // T3: Write a file on the FAT volume
  {
    auto file_result =
        vfs::Open("/mnt/fat/test.txt", vfs::kOCreate | vfs::kOReadWrite);
    EXPECT_TRUE(file_result.has_value(),
                "fatfs_system_test: open /mnt/fat/test.txt failed");
    vfs::File* file = file_result.value();

    const char kMsg[] = "Hello, FatFS!";
    auto write_result = vfs::Write(file, kMsg, sizeof(kMsg) - 1);
    EXPECT_TRUE(write_result.has_value(),
                "fatfs_system_test: write to /mnt/fat/test.txt failed");
    EXPECT_EQ(write_result.value(), sizeof(kMsg) - 1,
              "fatfs_system_test: write byte count mismatch");
    sk_printf("fatfs_system_test: wrote %zu bytes to /mnt/fat/test.txt\n",
              write_result.value());

    vfs::Close(file);
  }

  // T4: Read back and verify
  {
    auto file_result = vfs::Open("/mnt/fat/test.txt", vfs::kOReadOnly);
    EXPECT_TRUE(file_result.has_value(),
                "fatfs_system_test: re-open /mnt/fat/test.txt failed");
    vfs::File* file = file_result.value();

    char buf[64] = {};
    const char kMsg[] = "Hello, FatFS!";
    auto read_result = vfs::Read(file, buf, sizeof(buf) - 1);
    EXPECT_TRUE(read_result.has_value(),
                "fatfs_system_test: read from /mnt/fat/test.txt failed");
    EXPECT_EQ(read_result.value(), sizeof(kMsg) - 1,
              "fatfs_system_test: read byte count mismatch");
    EXPECT_EQ(std::memcmp(buf, kMsg, sizeof(kMsg) - 1), 0,
              "fatfs_system_test: read content mismatch");
    sk_printf("fatfs_system_test: verified read: %s\n", buf);

    vfs::Close(file);
  }

  // T5: MkDir on FAT volume
  {
    auto mkdir_result = vfs::MkDir("/mnt/fat/subdir");
    EXPECT_TRUE(mkdir_result.has_value(),
                "fatfs_system_test: mkdir /mnt/fat/subdir failed");
    sk_printf("fatfs_system_test: mkdir /mnt/fat/subdir ok\n");

    // Create a file inside subdir
    auto inner =
        vfs::Open("/mnt/fat/subdir/inner.txt", vfs::kOCreate | vfs::kOWriteOnly);
    EXPECT_TRUE(inner.has_value(),
                "fatfs_system_test: create /mnt/fat/subdir/inner.txt failed");
    if (inner.has_value()) {
      vfs::Close(inner.value());
    }

    // ReadDir on root of fat volume to find subdir
    auto dir_file = vfs::Open("/mnt/fat", vfs::kOReadOnly | vfs::kODirectory);
    EXPECT_TRUE(dir_file.has_value(),
                "fatfs_system_test: open /mnt/fat as dir failed");
    if (dir_file.has_value()) {
      vfs::DirEntry entries[16] = {};
      auto readdir_result = vfs::ReadDir(dir_file.value(), entries, 16);
      EXPECT_TRUE(readdir_result.has_value(),
                  "fatfs_system_test: readdir /mnt/fat failed");
      // Should see at least test.txt and subdir
      EXPECT_GT(readdir_result.value(), static_cast<size_t>(1),
                "fatfs_system_test: readdir /mnt/fat should return > 1 entry");
      sk_printf("fatfs_system_test: readdir /mnt/fat returned %zu entries\n",
                readdir_result.value());
      vfs::Close(dir_file.value());
    }
  }

  // T6: Unmount and remount — verify persistence
  {
    auto unmount_result = fat_fs.Unmount();
    EXPECT_TRUE(unmount_result.has_value(),
                "fatfs_system_test: FatFsFileSystem::Unmount failed");
    sk_printf("fatfs_system_test: unmounted ok\n");

    // Remount
    auto remount_result = fat_fs.Mount(blk);
    EXPECT_TRUE(remount_result.has_value(),
                "fatfs_system_test: remount failed");
    sk_printf("fatfs_system_test: remounted ok\n");

    // Re-wire VFS mount
    auto vfs_remount = vfs::GetMountTable().Mount("/mnt/fat", &fat_fs, blk);
    EXPECT_TRUE(vfs_remount.has_value(),
                "fatfs_system_test: vfs remount failed");

    // Verify test.txt persisted
    auto file_result = vfs::Open("/mnt/fat/test.txt", vfs::kOReadOnly);
    EXPECT_TRUE(file_result.has_value(),
                "fatfs_system_test: test.txt not found after remount");
    if (file_result.has_value()) {
      char buf[64] = {};
      const char kMsg[] = "Hello, FatFS!";
      auto read_result = vfs::Read(file_result.value(), buf, sizeof(buf) - 1);
      EXPECT_TRUE(read_result.has_value(),
                  "fatfs_system_test: read after remount failed");
      EXPECT_EQ(std::memcmp(buf, kMsg, sizeof(kMsg) - 1), 0,
                "fatfs_system_test: data corrupted after remount");
      sk_printf("fatfs_system_test: persistence verified: %s\n", buf);
      vfs::Close(file_result.value());
    }
  }

  sk_printf("fatfs_system_test: all tests passed\n");
  return true;
}
```

### Step 2: Verify it compiles (after CMakeLists.txt is updated in Task 4)

---

## Task 3: Update `tests/system_test/system_test.h`

**Files:**
- Modify: `tests/system_test/system_test.h:157` (append before `#endif`)

### Step 1: Add the two new declarations after line 156 (`auto exit_system_test() -> bool;`)

```cpp
auto ramfs_system_test() -> bool;
auto fatfs_system_test() -> bool;
```

Insert after line 156 (`auto exit_system_test() -> bool;`), before the blank line and `namespace MutexTest` block.

---

## Task 4: Update `tests/system_test/CMakeLists.txt`

**Files:**
- Modify: `tests/system_test/CMakeLists.txt`

### Step 1: Add the two new source files

After `exit_system_test.cpp` (line 30), add:

```cmake
    ramfs_system_test.cpp
    fatfs_system_test.cpp
```

### Step 2: Add filesystem to TARGET_LINK_LIBRARIES

Current line 41:
```cmake
TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE kernel_link_libraries libc
                                               libcxx arch driver task)
```

Change to:
```cmake
TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE kernel_link_libraries libc
                                               libcxx arch driver task filesystem)
```

### Step 3: Verify build compiles cleanly

```bash
cmake --preset build_riscv64
cd build_riscv64 && make system_test 2>&1 | head -60
```

Expected: no errors. Fix any missing include paths if needed.

---

## Task 5: Update `tests/system_test/main.cpp`

**Files:**
- Modify: `tests/system_test/main.cpp:49` (the `test_cases` array)

### Step 1: Change the array size and add entries

Current:
```cpp
std::array<test_case, 4> test_cases = {
    test_case{"thread_group_system_test", thread_group_system_test, false},
    test_case{"wait_system_test", wait_system_test, false},
    test_case{"clone_system_test", clone_system_test, false},
    test_case{"exit_system_test", exit_system_test, false}};
```

Change to:
```cpp
std::array<test_case, 6> test_cases = {
    test_case{"thread_group_system_test", thread_group_system_test, false},
    test_case{"wait_system_test", wait_system_test, false},
    test_case{"clone_system_test", clone_system_test, false},
    test_case{"exit_system_test", exit_system_test, false},
    test_case{"ramfs_system_test", ramfs_system_test, false},
    test_case{"fatfs_system_test", fatfs_system_test, false}};
```

---

## Task 6: Build and Run in QEMU

### Step 1: Full build

```bash
cmake --preset build_riscv64
cd build_riscv64 && make system_test
```

Expected: clean build, no warnings about missing symbols.

### Step 2: Run in QEMU

```bash
cd build_riscv64 && make system_test_run
```

### Step 3: Verify output in serial log

Check `build_riscv64/bin/system_test_qemu.log` for:

```
----ramfs_system_test----
ramfs_system_test: vfs init ok
ramfs_system_test: ramfs mounted at /
...
ramfs_system_test: all tests passed
----ramfs_system_test passed----
----fatfs_system_test----
fatfs_system_test: virtio-blk device: virtio-blk0
...
fatfs_system_test: all tests passed
----fatfs_system_test passed----
```

If `fatfs_system_test` shows `SKIP — no virtio-blk device available`, apply `2026-02-27-virtio-fatfs-plan.md` first.

---

## Task 7: Commit

```bash
git add tests/system_test/ramfs_system_test.cpp \
        tests/system_test/fatfs_system_test.cpp \
        tests/system_test/system_test.h \
        tests/system_test/CMakeLists.txt \
        tests/system_test/main.cpp \
        docs/plans/2026-02-27-filesystem-system-test.md
git commit --signoff -m "test(filesystem): add ramfs and fatfs system tests via VFS API"
```

---

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| `"vfs.hpp" not found` | `filesystem` not in include path | Verify `TARGET_LINK_LIBRARIES` includes `filesystem` in system_test `CMakeLists.txt` |
| `"block_device_provider.hpp" not found` | `block_device_provider.hpp` not created yet | Apply `2026-02-27-virtio-fatfs-plan.md` Task 1 first |
| `vfs::Init()` fails | VFS already initialized (idempotent call is fine) | Ignore second-call failures; check `has_value()` only on first call |
| `vfs::GetMountTable().Mount("/", ...)` fails | Already mounted in a previous test run / init | Each test run starts fresh in QEMU; if seen, check VFS init ordering |
| `FatFsFileSystem::Mount` returns error | `rootfs.img` not FAT32, or virtio-blk not probed | Check `2026-02-27-virtio-fatfs-plan.md` Task 5 (mkfs.fat), verify QEMU `-drive` flag |
| `readdir` returns 0 entries | `kODirectory` flag not recognized by ramfs | Check `vfs::Open` with `kODirectory`; fall back to opening as plain `kOReadOnly` if needed |
