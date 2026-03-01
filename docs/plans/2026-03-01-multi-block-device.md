# Multi-Block-Device Support (Plan A: DeviceNode field) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Support any number of virtio-blk devices by storing `vfs::BlockDevice*` inside `DeviceNode` and letting `filesystem.cpp` enumerate block devices via `DeviceManager::FindDevicesByType`, eliminating `GetVirtioBlkBlockDevice()`.

**Architecture:** Add `vfs::BlockDevice* block_device{nullptr}` to `DeviceNode`. After `VirtioBlk` creation in `VirtioDriver::Probe`, allocate a `VirtioBlkVfsAdapter` from a small static pool (no heap), store its pointer in the node. `filesystem.cpp` calls `FindDevicesByType(DeviceType::kBlock, ...)` to iterate all block devices. `GetVirtioBlkBlockDevice()` and `block_device_provider.hpp` are deleted.

**Tech Stack:** C++23 freestanding, ETL, `etl::optional`, static pool pattern (no heap), `Expected<T>` error handling.

---

## Key Files

| File | Change |
|------|--------|
| `src/device/include/device_node.hpp` | Add `vfs::BlockDevice* block_device{nullptr}` field |
| `src/device/virtio/virtio_driver.hpp` | Add static adapter pool; `Probe` writes to `node.block_device` |
| `src/device/virtio/virtio_driver.cpp` | After `blk_device_.emplace(...)`, construct adapter and assign `node.block_device` |
| `src/device/include/block_device_provider.hpp` | **Delete** |
| `src/device/device.cpp` | Remove `#include "block_device_provider.hpp"`, remove `GetVirtioBlkBlockDevice()` |
| `src/filesystem/filesystem.cpp` | Replace `GetVirtioBlkBlockDevice()` with `FindDevicesByType` loop |
| `src/device/virtio/device/virtio_blk_vfs_adapter.hpp` | Fix `GetName()` to return per-device name (optional polish) |

---

### Task 1: Add `block_device` field to `DeviceNode`

**Files:**
- Modify: `src/device/include/device_node.hpp`

**Step 1: Add forward declaration and field**

Add a forward declaration for `vfs::BlockDevice` before the struct, then add the field.

Before `struct DeviceNode {`, add:
```cpp
namespace vfs { class BlockDevice; }
```

Inside `struct DeviceNode`, after the `bool bound{false};` field, add:
```cpp
  /// Set by the driver's Probe() — points to a kernel-lifetime adapter object.
  /// nullptr if this device has not been probed or is not a block device.
  vfs::BlockDevice* block_device{nullptr};
```

**Step 2: Verify no include needed**

`vfs::BlockDevice*` is a pointer — forward declaration is sufficient. No `#include "block_device.hpp"` needed in `device_node.hpp`.

**Step 3: Check for compile errors**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | head -40
```

Expected: Compiles cleanly (or errors only in downstream files not yet updated).

---

### Task 2: Add static adapter pool to `VirtioDriver`

**Files:**
- Modify: `src/device/virtio/virtio_driver.hpp`

**Problem:** `VirtioBlkVfsAdapter` instances need kernel-lifetime storage. Currently there is one `static` local in `device.cpp`'s `GetVirtioBlkBlockDevice()`. For N devices, we need a static pool. Use a fixed-size array with a counter — freestanding, no heap.

**Step 1: Add pool constants and storage**

In `virtio_driver.hpp`, in the `private:` section, add after `uint32_t irq_{0};`:

```cpp
  // Static adapter pool — one slot per probed blk device (kernel lifetime).
  // kMaxBlkDevices should match or exceed the max virtio-blk devices in QEMU.
  static constexpr size_t kMaxBlkDevices = 4;
  static detail::virtio::blk::VirtioBlkVfsAdapter
      blk_adapters_[kMaxBlkDevices];
  static size_t blk_adapter_count_;
```

Note: `static` pool because `VirtioDriver` itself is a singleton. The adapters must outlive any filesystem mount.

**Step 2: Add definition to `virtio_driver.cpp`**

At the top of `virtio_driver.cpp`, after the existing includes, add:

```cpp
detail::virtio::blk::VirtioBlkVfsAdapter
    VirtioDriver::blk_adapters_[VirtioDriver::kMaxBlkDevices]{nullptr};
size_t VirtioDriver::blk_adapter_count_ = 0;
```

Wait — `VirtioBlkVfsAdapter` takes a `VirtioBlkType*` ctor argument. It can't be default-constructed. We need a different approach.

**Alternative: use placement new or etl::optional array**

Use `etl::optional<detail::virtio::blk::VirtioBlkVfsAdapter> blk_adapters_[kMaxBlkDevices]` (static, starts as `nullopt`):

```cpp
  static constexpr size_t kMaxBlkDevices = 4;
  static etl::optional<detail::virtio::blk::VirtioBlkVfsAdapter>
      blk_adapters_[kMaxBlkDevices];
  static size_t blk_adapter_count_;
```

In `virtio_driver.cpp`:
```cpp
etl::optional<detail::virtio::blk::VirtioBlkVfsAdapter>
    VirtioDriver::blk_adapters_[VirtioDriver::kMaxBlkDevices]{};
size_t VirtioDriver::blk_adapter_count_ = 0;
```

**Step 3: No compilation check yet** — wait for Task 3 to wire it all together.

---

### Task 3: Write `block_device` into `DeviceNode` in `Probe()`

**Files:**
- Modify: `src/device/virtio/virtio_driver.cpp`

**Step 1: After `blk_device_.emplace(std::move(*result));`, add the adapter**

In `virtio_driver.cpp`, inside `case DeviceId::kBlock:`, after:
```cpp
blk_device_.emplace(std::move(*result));
node.type = DeviceType::kBlock;
irq_ = node.irq;
```

Add:
```cpp
      // Allocate from static pool and expose as vfs::BlockDevice*
      if (blk_adapter_count_ < kMaxBlkDevices) {
        blk_adapters_[blk_adapter_count_].emplace(
            &blk_device_.value());
        node.block_device = &blk_adapters_[blk_adapter_count_].value();
        ++blk_adapter_count_;
      } else {
        klog::Warn(
            "VirtioDriver: blk adapter pool full, device at 0x%lX not "
            "accessible via block_device\n",
            ctx->base);
      }
```

**Step 2: Update `VirtioBlkVfsAdapter::GetName()` to include index (optional but nice)**

In `virtio_blk_vfs_adapter.hpp`, the `GetName()` currently returns hardcoded `"virtio-blk0"`. For multi-device, pass the index at construction time:

Change the constructor to:
```cpp
  explicit VirtioBlkVfsAdapter(VirtioBlkType* dev, uint32_t index = 0)
      : dev_(dev), index_(index) {}
```

And `GetName()` to return from a fixed name buffer:
```cpp
  [[nodiscard]] auto GetName() const -> const char* override {
    return name_;
  }
```

With a `char name_[16]` field initialized in the constructor using `kstd::snprintf` or manual construction.

Actually, to keep it simple (freestanding, no snprintf complexity), just store the index and build the name lazily, or use a static per-index name table:

```cpp
  [[nodiscard]] auto GetName() const -> const char* override {
    static const char* const kNames[] = {
        "virtio-blk0", "virtio-blk1", "virtio-blk2", "virtio-blk3"};
    return (index_ < 4) ? kNames[index_] : "virtio-blk?";
  }
```

And pass `blk_adapter_count_` as the index when constructing:
```cpp
blk_adapters_[blk_adapter_count_].emplace(
    &blk_device_.value(), static_cast<uint32_t>(blk_adapter_count_));
```

**Step 3: Build to verify**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | head -60
```

Expected: Compiles cleanly.

---

### Task 4: Update `filesystem.cpp` to use `FindDevicesByType`

**Files:**
- Modify: `src/filesystem/filesystem.cpp`

**Step 1: Replace `GetVirtioBlkBlockDevice()` call with enumeration**

Remove:
```cpp
#include "block_device_provider.hpp"
```

Replace the FatFS mount block (lines 34–51 approx):
```cpp
  // Mount FatFS on virtio-blk0 at /mnt/fat
  auto* blk = GetVirtioBlkBlockDevice();
  if (blk != nullptr) {
    static fatfs::FatFsFileSystem fat_fs(kRootFsDriveId);
    auto fat_mount = fat_fs.Mount(blk);
    ...
  }
```

With:
```cpp
  // Mount FatFS on the first available block device at /mnt/fat
  DeviceNode* blk_nodes[4]{};
  size_t blk_count = DeviceManagerSingleton::instance().FindDevicesByType(
      DeviceType::kBlock, blk_nodes, 4);

  if (blk_count > 0 && blk_nodes[0]->block_device != nullptr) {
    auto* blk = blk_nodes[0]->block_device;
    static fatfs::FatFsFileSystem fat_fs(kRootFsDriveId);
    auto fat_mount = fat_fs.Mount(blk);
    if (!fat_mount.has_value()) {
      klog::Err("FileSystemInit: FatFsFileSystem::Mount failed: %s\n",
                fat_mount.error().message());
    } else {
      auto vfs_mount = vfs::GetMountTable().Mount("/mnt/fat", &fat_fs, blk);
      if (!vfs_mount.has_value()) {
        klog::Err("FileSystemInit: vfs mount at /mnt/fat failed: %s\n",
                  vfs_mount.error().message());
      } else {
        klog::Info("FileSystemInit: FatFS mounted at /mnt/fat (device: %s)\n",
                   blk->GetName());
      }
    }
  }
```

**Step 2: Add missing includes**

Add to `filesystem.cpp` includes:
```cpp
#include "device_manager.hpp"
#include "device_node.hpp"
```

**Step 3: Build**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | head -60
```

Expected: Compiles cleanly.

---

### Task 5: Delete `block_device_provider.hpp` and `GetVirtioBlkBlockDevice()`

**Files:**
- Delete: `src/device/include/block_device_provider.hpp`
- Modify: `src/device/device.cpp` — remove `#include "block_device_provider.hpp"` and the `GetVirtioBlkBlockDevice()` function

**Step 1: Verify no remaining references**

```bash
grep -r "block_device_provider\|GetVirtioBlkBlockDevice" /home/nzh/MRNIU/SimpleKernel/src /home/nzh/MRNIU/SimpleKernel/tests
```

Expected: zero results (after Task 4 is done).

**Step 2: Remove from `device.cpp`**

Remove from `device.cpp`:
- The `#include "block_device_provider.hpp"` line
- The entire `GetVirtioBlkBlockDevice()` function (lines 44–52)

**Step 3: Delete the header file**

```bash
rm src/device/include/block_device_provider.hpp
```

**Step 4: Check `fatfs/diskio.cpp` for any direct include**

```bash
grep -r "block_device_provider\|GetVirtioBlkBlockDevice" /home/nzh/MRNIU/SimpleKernel/src/filesystem
```

If `diskio.cpp` also calls `GetVirtioBlkBlockDevice`, update it similarly to Task 4.

**Step 5: Final build**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | head -60
```

Expected: Clean build.

---

### Task 6: Final verification

**Step 1: Check for pre-commit compliance**

```bash
cd /home/nzh/MRNIU/SimpleKernel && pre-commit run --all-files 2>&1 | tail -30
```

Fix any clang-format issues if present.

**Step 2: Grep for old references**

```bash
grep -r "GetVirtioBlkBlockDevice\|block_device_provider" /home/nzh/MRNIU/SimpleKernel/src /home/nzh/MRNIU/SimpleKernel/tests
```

Expected: zero results.

**Step 3: QEMU smoke test (optional — requires QEMU environment)**

```bash
cd build_riscv64 && make run
```

Expected: `FileSystemInit: FatFS mounted at /mnt/fat (device: virtio-blk0)` in QEMU output.

---

## Design Notes

### Why static pool instead of heap?
The kernel's memory subsystem is initialized before `DeviceInit()`, so heap allocation via `kstd::make_unique` is technically available (as seen in `VirtioDriver::Probe` with `dma_buffer_`). However, a static pool makes the lifetime contract explicit and avoids any ordering subtleties. For 4 max devices, the static cost is negligible.

### Why `etl::optional` array instead of raw array?
`VirtioBlkVfsAdapter` requires a non-null `VirtioBlkType*` at construction — it cannot be default-constructed. `etl::optional<VirtioBlkVfsAdapter>` starts as `nullopt` and is emplaced in-place when the device is probed, giving us value semantics without heap.

### Why keep `blk_adapters_` and `blk_adapter_count_` as `static` members (not instance)?
`VirtioDriver` is a singleton. The distinction between `static` and instance member here is cosmetic. Instance members are chosen for consistency with other fields (`blk_device_`, `dma_buffer_`, `irq_`).

Actually, re-evaluating: `blk_device_` is already a single `etl::optional` — but with multiple virtio-blk nodes, Probe is called once per node and `blk_device_` would be overwritten each time. The pool approach above fixes this. Keep `blk_adapters_` and `blk_adapter_count_` as **instance members** for consistency.

### `GetBlkDevice()` on `VirtioDriver`
The method `GetBlkDevice()` returns the single `blk_device_`. After this refactor, `GetVirtioBlkBlockDevice()` is gone and callers go through `DeviceNode::block_device`. The `GetBlkDevice()` method can be removed too, but it's safe to leave it for now (it's not called anywhere after Task 5).
