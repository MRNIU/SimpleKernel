# VirtioBlk ↔ FatFS Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire the already-probed `VirtioBlkDriver` to `FatFsFileSystem` so that `rootfs.img` (FAT32) is mounted at `/mnt/fat` in the VFS.

**Architecture:** A thin `VirtioBlkBlockDevice` adapter (anonymous namespace in `device.cpp`) adapts `VirtioBlk<PlatformTraits>` to the `vfs::BlockDevice` pure-virtual interface. A free function `GetVirtioBlkBlockDevice()` is declared in a new header `block_device_provider.hpp` and implemented in `device.cpp`, isolating the filesystem layer from virtio internals. `filesystem.cpp` calls this function and mounts FatFS. CMake auto-formats `rootfs.img` as FAT32 after `dd`.

**Tech Stack:** C++23 · `device_framework::virtio::blk::VirtioBlk<PlatformTraits>` · `vfs::BlockDevice` · `fatfs::FatFsFileSystem` · CMake `ADD_CUSTOM_COMMAND`

---

## Reference: Key Types and APIs

Before starting, understand these:

```cpp
// 3rd/device_framework/include/device_framework/detail/virtio/device/virtio_blk_device.hpp
// VirtioBlk<Traits>::Read(uint64_t lba, uint8_t* buf)  -> device_framework::Expected<void>  (one sector = 512B)
// VirtioBlk<Traits>::Write(uint64_t lba, const uint8_t* buf) -> device_framework::Expected<void>
// VirtioBlk<Traits>::GetCapacity() -> uint64_t  (sector count)

// src/device/include/df_bridge.hpp
// df_bridge::ToKernelError(device_framework::Error) -> Error  (converts df error to kernel error)

// src/device/include/driver_registry.hpp
// DriverRegistry::GetDriverInstance<Driver>()  -> Driver&   (returns static driver singleton)
// After Probe succeeds, VirtioBlkDriver<PlatformTraits>::GetDevice() -> VirtioBlkType*

// src/filesystem/fatfs/include/fatfs.hpp
// fatfs::FatFsFileSystem::Mount(vfs::BlockDevice*) -> Expected<vfs::Inode*>
// Constructor: FatFsFileSystem(uint8_t drive_number)

// src/filesystem/vfs/include/mount.hpp
// vfs::GetMountTable().Mount(const char* path, vfs::FileSystem*, vfs::BlockDevice*) -> Expected<vfs::Inode*>
```

---

### Task 1: Create `src/device/include/block_device_provider.hpp`

**Files:**
- Create: `src/device/include/block_device_provider.hpp`

**Step 1: Write the new header file**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_PROVIDER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_PROVIDER_HPP_

#include "block_device.hpp"

/**
 * @brief Returns the kernel vfs::BlockDevice* for the first probed virtio-blk
 *        device, or nullptr if no device has been probed yet.
 *
 * @pre  DeviceInit() has been called and VirtioBlkDriver probe succeeded.
 * @post Returns a pointer to a static adapter object whose lifetime is the
 *       kernel lifetime. The pointer must not be deleted.
 */
auto GetVirtioBlkBlockDevice() -> vfs::BlockDevice*;

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_PROVIDER_HPP_ */
```

**Step 2: Verify the file exists and compiles standalone**

The header only includes `"block_device.hpp"` (already in the include path). No build step needed for the header alone—just confirm it looks correct.

---

### Task 2: Add `VirtioBlkBlockDevice` adapter + `GetVirtioBlkBlockDevice()` to `device.cpp`

**Files:**
- Modify: `src/device/device.cpp`

**Step 1: Read the current `device.cpp`**

```bash
cat src/device/device.cpp
```

Note the existing includes and the structure of `DeviceInit()`. You will append *after* the existing code (do not touch `DeviceInit()`).

**Step 2: Add includes at the top of `device.cpp`**

Add these lines after the existing project includes (preserve include ordering: stdlib → third-party → project):

```cpp
#include "block_device_provider.hpp"
#include "df_bridge.hpp"
#include "driver/virtio_blk_driver.hpp"
#include "platform_traits.hpp"
```

**Step 3: Add the anonymous-namespace adapter class + free function**

Append at the **end** of `device.cpp`, after `DeviceInit()`:

```cpp
namespace {

/// @brief Adapts VirtioBlk<PlatformTraits> to vfs::BlockDevice.
class VirtioBlkBlockDevice final : public vfs::BlockDevice {
 public:
  using VirtioBlkType =
      typename VirtioBlkDriver<PlatformTraits>::VirtioBlkType;

  explicit VirtioBlkBlockDevice(VirtioBlkType* dev) : dev_(dev) {}

  auto ReadSectors(uint64_t lba, uint32_t count, void* buf)
      -> Expected<size_t> override {
    auto* ptr = static_cast<uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto result = dev_->Read(lba + i, ptr + i * kSectorSize);
      if (!result) {
        return std::unexpected(df_bridge::ToKernelError(result.error()));
      }
    }
    return static_cast<size_t>(count) * kSectorSize;
  }

  auto WriteSectors(uint64_t lba, uint32_t count, const void* buf)
      -> Expected<size_t> override {
    const auto* ptr = static_cast<const uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto result =
          dev_->Write(lba + i, ptr + i * kSectorSize);
      if (!result) {
        return std::unexpected(df_bridge::ToKernelError(result.error()));
      }
    }
    return static_cast<size_t>(count) * kSectorSize;
  }

  [[nodiscard]] auto GetSectorSize() const -> uint32_t override {
    return kSectorSize;
  }

  [[nodiscard]] auto GetSectorCount() const -> uint64_t override {
    return dev_->GetCapacity();
  }

  [[nodiscard]] auto GetName() const -> const char* override {
    return "virtio-blk0";
  }

 private:
  static constexpr uint32_t kSectorSize = 512;
  VirtioBlkType* dev_;
};

}  // namespace

auto GetVirtioBlkBlockDevice() -> vfs::BlockDevice* {
  using Driver = VirtioBlkDriver<PlatformTraits>;
  auto* raw =
      Singleton<DriverRegistry>::GetInstance()
          .template GetDriverInstance<Driver>()
          .GetDevice();
  if (raw == nullptr) {
    klog::Err("GetVirtioBlkBlockDevice: no virtio-blk device probed\n");
    return nullptr;
  }
  static VirtioBlkBlockDevice adapter(raw);
  return &adapter;
}
```

> **Note on `GetDriverInstance` API**: check `src/device/include/driver_registry.hpp`. If `GetDriverInstance<D>()` is a member of `DriverRegistry`, call it as shown; adjust if it's a static free function.

**Step 4: Build to check for compile errors**

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel 2>&1 | head -60
```

Expected: no errors related to `device.cpp`. Fix any include-path or API mismatches before proceeding.

---

### Task 3: Mount FatFS in `filesystem.cpp`

**Files:**
- Modify: `src/filesystem/filesystem.cpp`

**Step 1: Add includes at the top**

After the existing includes (`kernel_log.hpp`, `mount.hpp`, `ramfs.hpp`, `vfs.hpp`), add:

```cpp
#include "block_device_provider.hpp"
#include "fatfs.hpp"
```

**Step 2: Append FatFS mount logic at the end of `FileSystemInit()`**

Current last line: `klog::Info("FileSystemInit: complete\n");`

Replace it with:

```cpp
  // Mount FatFS on virtio-blk0 at /mnt/fat
  auto* blk = GetVirtioBlkBlockDevice();
  if (blk != nullptr) {
    static fatfs::FatFsFileSystem fat_fs(0);
    auto fat_mount = fat_fs.Mount(blk);
    if (!fat_mount.has_value()) {
      klog::Err("FileSystemInit: FatFsFileSystem::Mount failed: %s\n",
                fat_mount.error().message());
    } else {
      auto vfs_mount =
          vfs::GetMountTable().Mount("/mnt/fat", &fat_fs, blk);
      if (!vfs_mount.has_value()) {
        klog::Err("FileSystemInit: vfs mount at /mnt/fat failed: %s\n",
                  vfs_mount.error().message());
      } else {
        klog::Info("FileSystemInit: FatFS mounted at /mnt/fat\n");
      }
    }
  }

  klog::Info("FileSystemInit: complete\n");
```

**Step 3: Build**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | head -60
```

Expected: no errors. If `"block_device_provider.hpp"` is not found, Task 4 will fix the include path—come back here after Task 4.

---

### Task 4: Add `device/include` to `filesystem` target include paths

**Files:**
- Modify: `src/filesystem/CMakeLists.txt`

**Step 1: Edit line 5**

Current:
```cmake
TARGET_INCLUDE_DIRECTORIES (filesystem INTERFACE include)
```

Change to:
```cmake
TARGET_INCLUDE_DIRECTORIES (
    filesystem INTERFACE include
    ${CMAKE_SOURCE_DIR}/src/device/include)
```

**Step 2: Build again**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | head -60
```

Expected: clean build.

---

### Task 5: Auto-format `rootfs.img` as FAT32 in CMake

**Files:**
- Modify: `cmake/functions.cmake` (lines 79–85)

**Step 1: Locate the `rootfs.img` custom command**

Current (lines 79–85):
```cmake
    # 生成 rootfs.img
    ADD_CUSTOM_COMMAND (
        OUTPUT ${CMAKE_BINARY_DIR}/bin/rootfs.img
        COMMENT "Generating rootfs.img ..."
        VERBATIM
        WORKING_DIRECTORY $<TARGET_FILE_DIR:${ARG_TARGET}>
        COMMAND dd if=/dev/zero of=${CMAKE_BINARY_DIR}/bin/rootfs.img bs=1M
                count=64)
```

**Step 2: Add `mkfs.fat` as a second COMMAND**

```cmake
    # 生成 rootfs.img
    ADD_CUSTOM_COMMAND (
        OUTPUT ${CMAKE_BINARY_DIR}/bin/rootfs.img
        COMMENT "Generating rootfs.img ..."
        VERBATIM
        WORKING_DIRECTORY $<TARGET_FILE_DIR:${ARG_TARGET}>
        COMMAND dd if=/dev/zero of=${CMAKE_BINARY_DIR}/bin/rootfs.img bs=1M
                count=64
        COMMAND mkfs.fat -F 32 ${CMAKE_BINARY_DIR}/bin/rootfs.img)
```

> **Prerequisite**: `dosfstools` must be installed in the build environment.
> Docker image: `docker exec SimpleKernel-dev apt-get install -y dosfstools`
> Local: `sudo apt-get install -y dosfstools`

**Step 3: Delete the old `rootfs.img` so CMake regenerates it**

```bash
rm -f build_riscv64/bin/rootfs.img
```

**Step 4: Rebuild**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

Expected output includes:
```
Generating rootfs.img ...
mkfs.fat 4.x ...
```

---

### Task 6: End-to-End Verification in QEMU

**Step 1: Run the kernel**

```bash
cd build_riscv64 && make run
```

**Step 2: Check serial output**

Look for these lines (order may vary):

```
FileSystemInit: FatFS mounted at /mnt/fat
FileSystemInit: complete
```

**Step 3: Confirm no regressions**

- RamFS at `/` still mounts (log line unchanged)
- No kernel panics or infinite loops
- If FatFS mount fails with an error message, check:
  - Was `VirtioBlkDriver` probed? Look for `VirtioBlk: probe` in earlier log lines.
  - Is `rootfs.img` present and FAT32-formatted? `file build_riscv64/bin/rootfs.img` should say `FAT (32 bit)`.

---

### Task 7: Commit

```bash
git add src/device/include/block_device_provider.hpp \
        src/device/device.cpp \
        src/filesystem/filesystem.cpp \
        src/filesystem/CMakeLists.txt \
        cmake/functions.cmake
git commit --signoff -m "feat(device,filesystem): wire virtio-blk to FatFS via block_device_provider"
```

---

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| `GetVirtioBlkBlockDevice` link error | `device.cpp` not in `filesystem` link chain | Check `TARGET_LINK_LIBRARIES` in `src/filesystem/CMakeLists.txt`; may need to link `device` library |
| `"fatfs.hpp" not found` | Include path missing | Verify `fatfs` subdirectory adds its `include/` to the interface; check `src/filesystem/fatfs/CMakeLists.txt` |
| `FatFsFileSystem::Mount` returns error | `rootfs.img` not FAT32 | Delete `rootfs.img` and rebuild; verify `mkfs.fat` ran |
| `vfs::GetMountTable().Mount("/mnt/fat")` fails | `/mnt/fat` directory doesn't exist in VFS | May need to create `/mnt` and `/mnt/fat` inodes in ramfs before calling VFS mount |
| `GetDevice()` returns nullptr | `VirtioBlkDriver` probe not called or failed | Check `DeviceInit()` registers and probes the driver; verify QEMU `-drive` flag includes `rootfs.img` |
