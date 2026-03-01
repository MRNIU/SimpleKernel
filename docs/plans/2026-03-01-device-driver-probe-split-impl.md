# Device Driver-Probe Split Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Separate hardware detection (`Match()`) from driver initialization (`Probe()`), and move DMA buffer ownership from `DeviceNode` into `VirtioBlkDriver`.

**Architecture:** Add `Match(DeviceNode&) -> bool` to the `Driver` concept and `DriverEntry`. `ProbeAll()` calls `match()` before `probe()`. `VirtioBlkDriver::Probe()` self-allocates its DMA buffer. `DeviceNode::dma_buffer` field is removed entirely.

**Tech Stack:** C++23, freestanding kernel, header-only device framework, `Expected<T>`, `kstd::unique_ptr`, `etl::io_port_ro`

---

## Context for Every Task

All files live under `/home/nzh/MRNIU/SimpleKernel/`.

Key invariants:
- **No exceptions, RTTI, `dynamic_cast`, `typeid`**
- **Header-only framework** — `src/device/include/*.hpp` and `driver/*.hpp` stay header-only; only `device.cpp` is a `.cpp`
- **Google C++ style** — trailing return types `auto Foo() -> RetType`, members end with `_`, constants `kCamelCase`
- **`Expected<T>`** = `std::expected<T, Error>` alias — use `std::unexpected(Error(...))` for errors
- All files have `#ifndef SIMPLEKERNEL_SRC_...` guards and `/** @copyright Copyright The SimpleKernel Contributors */`
- The `device_framework` submodule under `3rd/` is **NOT modified**

---

## Task 1: Add `Match()` to the `Driver` concept and `DriverEntry` in `driver_registry.hpp`

**Files:**
- Modify: `src/device/include/driver_registry.hpp`

### Step 1: Read the current file

```bash
# Already read — lines 52-64 define Driver concept, lines 60-64 define DriverEntry
```

The current `Driver` concept (lines 52–57):
```cpp
template <typename D>
concept Driver = requires(D d, DeviceNode& node) {
  { D::GetDescriptor() } -> std::same_as<const DriverDescriptor&>;
  { d.Probe(node) } -> std::same_as<Expected<void>>;
  { d.Remove(node) } -> std::same_as<Expected<void>>;
};
```

The current `DriverEntry` (lines 60–64):
```cpp
struct DriverEntry {
  const DriverDescriptor* descriptor;
  auto (*probe)(DeviceNode&) -> Expected<void>;
  auto (*remove)(DeviceNode&) -> Expected<void>;
};
```

The current `Register<D>()` lambda block (lines 103–112):
```cpp
drivers_[idx] = DriverEntry{
    .descriptor = &D::GetDescriptor(),
    .probe = [](DeviceNode& node) -> Expected<void> {
      return GetDriverInstance<D>().Probe(node);
    },
    .remove = [](DeviceNode& node) -> Expected<void> {
      return GetDriverInstance<D>().Remove(node);
    },
};
```

### Step 2: Make the edits

**Edit A — Driver concept:** Add `{ d.Match(node) } -> std::same_as<bool>;` between `GetDescriptor` and `Probe` requirements.

**Edit B — DriverEntry:** Add `auto (*match)(DeviceNode&) -> bool;` as the second field (after `descriptor`).

**Edit C — Register<D>() lambda block:** Add `.match` lambda binding:
```cpp
drivers_[idx] = DriverEntry{
    .descriptor = &D::GetDescriptor(),
    .match  = [](DeviceNode& n) -> bool          { return GetDriverInstance<D>().Match(n); },
    .probe  = [](DeviceNode& n) -> Expected<void> { return GetDriverInstance<D>().Probe(n); },
    .remove = [](DeviceNode& n) -> Expected<void> { return GetDriverInstance<D>().Remove(n); },
};
```

Note: rename parameter from `node` to `n` in the lambdas for consistency — or keep `node`, either is fine.

### Step 3: Verify — check for syntax errors

Run:
```bash
cd /home/nzh/MRNIU/SimpleKernel && cmake --preset build_riscv64 2>&1 | head -40
```
Expected: No errors mentioning `driver_registry.hpp`. (Full kernel build comes in Task 7.)

---

## Task 2: Add `Match()` call to `ProbeAll()` in `device_manager.hpp`

**Files:**
- Modify: `src/device/include/device_manager.hpp`

### Step 1: Locate the insertion point

Current `ProbeAll()` body (lines 61–95). The match-and-probe sequence is:
```cpp
auto* drv = registry_.FindDriver(devices_[i].resource);
if (drv == nullptr) { ... continue; }

// TryBind() ...
if (!devices_[i].TryBind()) continue;

auto result = drv->probe(devices_[i]);
```

### Step 2: Insert `drv->match()` check between `FindDriver` and `TryBind`

After the `drv == nullptr` check and before `TryBind()`, insert:
```cpp
    // Fine hardware detection — driver reads registers to confirm identity
    if (!drv->match(devices_[i])) {
      klog::Debug("DeviceManager: '%s' hardware detection rejected by '%s'\n",
                  devices_[i].name, drv->descriptor->name);
      continue;
    }
```

### Step 3: Verify — check for syntax errors

```bash
cd /home/nzh/MRNIU/SimpleKernel && cmake --preset build_riscv64 2>&1 | head -40
```
Expected: No new errors from `device_manager.hpp`.

---

## Task 3: Add `Match()` to `Ns16550aDriver` in `ns16550a_driver.hpp`

**Files:**
- Modify: `src/device/include/driver/ns16550a_driver.hpp`

### Step 1: Locate insertion point

Current class body has `Probe()` at line 36. Add `Match()` immediately before `Probe()`.

### Step 2: Insert `Match()` method

```cpp
  auto Match([[maybe_unused]] DeviceNode& node) -> bool { return true; }
```

The NS16550A compatible strings (`"ns16550a"`, `"ns16550"`) uniquely identify this device — no register reads needed.

### Step 3: Verify — cmake check

```bash
cd /home/nzh/MRNIU/SimpleKernel && cmake --preset build_riscv64 2>&1 | head -40
```

---

## Task 4: Refactor `VirtioBlkDriver` — add `Match()`, move DMA to `Probe()`

**Files:**
- Modify: `src/device/include/driver/virtio_blk_driver.hpp`

### Step 1: Understand the current structure

- Lines 41–117: `Probe()` — currently reads magic + device_id, checks `node.dma_buffer`, calls `VirtioBlkType::Create()`
- Lines 138–141: private fields — `device_`, `irq_`
- `kMinDmaBufferSize = 32768` is already a public constexpr (line 30)
- `kBlockDeviceId = 2` is currently a local inside `Probe()` (line 57) — make it a private `static constexpr`

### Step 2: Add `kBlockDeviceId` as a private static constexpr

Add to private section:
```cpp
  static constexpr uint32_t kBlockDeviceId = 2;
```

Remove the local `constexpr uint32_t kBlockDeviceId = 2;` declaration from `Probe()`.

### Step 3: Add the `Match()` method (insert before `Probe()`)

```cpp
  /// @brief Hardware detection — reads magic + device_id registers to confirm
  ///        this is a VirtIO block device. Does NOT map MMIO or modify state.
  auto Match(DeviceNode& node) -> bool {
    if (node.resource.mmio_count == 0) return false;
    uint64_t base = node.resource.mmio[0].base;
    if (base == 0) return false;
    etl::io_port_ro<uint32_t> magic{reinterpret_cast<void*>(base)};
    if (magic.read() != device_framework::virtio::kMmioMagicValue) return false;
    etl::io_port_ro<uint32_t> dev_id{reinterpret_cast<void*>(
        base +
        device_framework::virtio::MmioTransport::MmioReg::kDeviceId)};
    return dev_id.read() == kBlockDeviceId;
  }
```

### Step 4: Rewrite `Probe()` — remove hardware detection, add self-managed DMA

Replace the current `Probe()` body. The new `Probe()` assumes `Match()` already confirmed device identity. It:
1. Calls `df_bridge::PrepareMmioProbe()` for virtual MMIO mapping
2. Allocates its own DMA buffer (no longer from `node.dma_buffer`)
3. Calls `VirtioBlkType::Create()`
4. Stores DMA buffer in `dma_buf_` private field

New `Probe()`:
```cpp
  auto Probe(DeviceNode& node) -> Expected<void> {
    auto ctx = df_bridge::PrepareMmioProbe(node, kMmioRegionSize);
    if (!ctx) {
      return std::unexpected(ctx.error());
    }

    auto base = ctx->base;

    // DMA buffer is now driver-owned; allocated here after Match() confirmed identity
    auto dma_buf = kstd::make_unique<IoBuffer>(kMinDmaBufferSize);
    if (!dma_buf || !dma_buf->IsValid()) {
      klog::Err("VirtioBlkDriver: failed to allocate DMA buffer at 0x%lX\n",
                base);
      node.Release();
      return std::unexpected(Error(ErrorCode::kOutOfMemory));
    }

    if (dma_buf->GetBuffer().size() < kMinDmaBufferSize) {
      klog::Err("VirtioBlkDriver: DMA buffer too small (%zu < %u)\n",
                dma_buf->GetBuffer().size(), kMinDmaBufferSize);
      node.Release();
      return std::unexpected(Error(ErrorCode::kInvalidArgument));
    }

    uint64_t extra_features =
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kSegMax) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kSizeMax) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kBlkSize) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kFlush) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kGeometry);

    auto result = VirtioBlkType::Create(base, dma_buf->GetBuffer().data(),
                                        kDefaultQueueCount, kDefaultQueueSize,
                                        extra_features);
    if (!result.has_value()) {
      klog::Err("VirtioBlkDriver: Create failed at 0x%lX\n", base);
      node.Release();
      return std::unexpected(Error(result.error().code));
    }

    device_.Emplace(std::move(*result));
    dma_buf_ = std::move(dma_buf);  // driver owns DMA buffer lifetime
    node.type = DeviceType::kBlock;

    if (node.resource.irq_count > 0) {
      irq_ = node.resource.irq[0];
    }

    auto capacity = device_.Value().GetCapacity();
    klog::Info(
        "VirtioBlkDriver: block device at 0x%lX, capacity=%lu sectors, "
        "irq=%u\n",
        base, capacity, irq_);

    return {};
  }
```

### Step 5: Add `dma_buf_` to private fields

In the private section, after `device_` and before `irq_`:
```cpp
  kstd::unique_ptr<IoBuffer> dma_buf_;  // driver-owned DMA buffer
```

Add the required include at the top of the file if not already present:
```cpp
#include "io_buffer.hpp"
#include "kstd_memory"
```

Check whether these are already transitively included. If `device_node.hpp` was previously including them, they may now be missing — add them explicitly.

### Step 6: Verify — cmake check

```bash
cd /home/nzh/MRNIU/SimpleKernel && cmake --preset build_riscv64 2>&1 | head -40
```

---

## Task 5: Remove `dma_buffer` from `DeviceNode` in `device_node.hpp`

**Files:**
- Modify: `src/device/include/device_node.hpp`

### Step 1: Understand what to remove

Currently in `DeviceNode`:
- Line 8: `#include <etl/memory.h>` — used only for `etl::unique_ptr<IoBuffer>`
- Line 14: `#include "io_buffer.hpp"` — used only for `IoBuffer`
- Line 16: `#include "kstd_memory"` — check whether it's used elsewhere in this file
- Line 107: `etl::unique_ptr<IoBuffer> dma_buffer{};` — the field itself
- Line 141: `dma_buffer(std::move(other.dma_buffer))` — in move constructor
- Line 154: `dma_buffer = std::move(other.dma_buffer);` — in move assignment

### Step 2: Check `kstd_memory` usage

`kstd_memory` provides `kstd::memcpy`. It IS used in the move constructor and move assignment (`kstd::memcpy(name, ...)`). **Do NOT remove `kstd_memory`.**

### Step 3: Remove `dma_buffer`-related code

1. Remove `#include <etl/memory.h>` (line 8) — only used for `etl::unique_ptr`
2. Remove `#include "io_buffer.hpp"` (line 14) — only used for `IoBuffer`
3. Remove the field: `etl::unique_ptr<IoBuffer> dma_buffer{};` (line 107) and its comment (line 106)
4. In the move constructor, remove `dma_buffer(std::move(other.dma_buffer))` from the initializer list (line 141)
5. In the move assignment, remove `dma_buffer = std::move(other.dma_buffer);` (line 154)

### Step 4: Verify — cmake check

```bash
cd /home/nzh/MRNIU/SimpleKernel && cmake --preset build_riscv64 2>&1 | head -40
```
Expected: No errors from `device_node.hpp`. Errors from `device.cpp` are expected (we haven't updated it yet).

---

## Task 6: Clean up `device.cpp` — remove DMA loop and related code

**Files:**
- Modify: `src/device/device.cpp`

### Step 1: Identify what to remove

Current `DeviceInit()` (lines 18–80):

Lines to **remove**:
- Line 14: `#include "kstd_memory"` — no longer needed in this file
- Lines 47–69: the entire DMA pre-allocation block:
  ```cpp
  // 为 virtio,mmio 设备分配 DMA 缓冲区
  constexpr size_t kMaxPlatformDevices = 64;
  DeviceNode* devs[kMaxPlatformDevices];
  size_t n = dm.FindDevicesByType(...);
  for (size_t i = 0; i < n; ++i) {
    ...
  }
  ```

Lines to **keep**:
- `#include "kstd_cstring"` if present (check — it may be `strcmp` that's gone with the loop)
- All driver registration and bus registration code (lines 23–45)
- `dm.ProbeAll()` call (line 72)
- The `GetVirtioBlkBlockDevice()` function and `VirtioBlkBlockDevice` adapter class (lines 82–143) — **preserve entirely**

Also check: does `strcmp` appear anywhere else in `device.cpp` after the loop is removed? If not, and if `kstd_cstring` was included only for `strcmp`, remove that include too. Look at line 7 — `#include "block_device_provider.hpp"` — that one stays.

### Step 2: Make the edits

Remove the `#include "kstd_memory"` line.

Remove lines 47–69 (the DMA allocation block including its comment).

The result should be a clean `DeviceInit()`:
```cpp
auto DeviceInit() -> void {
  DeviceManagerSingleton::create();
  auto& dm = DeviceManagerSingleton::instance();
  auto& fdt = KernelFdtSingleton::instance();

  // 注册驱动
  auto reg_uart = dm.GetRegistry().Register<Ns16550aDriver>();
  if (!reg_uart.has_value()) {
    klog::Err("DeviceInit: failed to register Ns16550aDriver: %s\n",
              reg_uart.error().message());
    return;
  }

  auto reg_blk = dm.GetRegistry().Register<VirtioBlkDriver>();
  if (!reg_blk.has_value()) {
    klog::Err("DeviceInit: failed to register VirtioBlkDriver: %s\n",
              reg_blk.error().message());
    return;
  }

  // 注册 Platform 总线并枚举设备
  PlatformBus platform_bus(fdt);
  auto bus_result = dm.RegisterBus(platform_bus);
  if (!bus_result.has_value()) {
    klog::Err("DeviceInit: PlatformBus enumeration failed: %s\n",
              bus_result.error().message());
    return;
  }

  // 匹配驱动并 Probe
  auto probe_result = dm.ProbeAll();
  if (!probe_result.has_value()) {
    klog::Err("DeviceInit: ProbeAll failed: %s\n",
              probe_result.error().message());
    return;
  }

  klog::Info("DeviceInit: complete\n");
}
```

### Step 3: Verify — cmake check

```bash
cd /home/nzh/MRNIU/SimpleKernel && cmake --preset build_riscv64 2>&1 | head -60
```

---

## Task 7: Full build verification

### Step 1: Run cmake preset (generates build system)

```bash
cmake --preset build_riscv64
```
Expected: `-- Configuring done` / `-- Build files have been written to: .../build_riscv64`

### Step 2: Build the kernel

```bash
cd /home/nzh/MRNIU/SimpleKernel/build_riscv64 && make SimpleKernel 2>&1
```
Expected: Exit code 0, binary produced at `build_riscv64/bin/SimpleKernel`.

If errors appear, read the error carefully. Common failure modes:
- `Match` not found → Task 1 or 3/4 edit missed something
- `dma_buffer` still referenced → Task 5 missed a reference
- `IoBuffer` not found in `virtio_blk_driver.hpp` → add missing includes

### Step 3: Fix any compile errors

If Step 2 fails, fix the specific error and re-run `make SimpleKernel`.

---

## Task 8: Functional verification (grep checks)

### Step 1: Confirm `device.cpp` has no VirtIO-specific strings

```bash
grep -n "virtio,mmio" /home/nzh/MRNIU/SimpleKernel/src/device/device.cpp
```
Expected: **zero results**

### Step 2: Confirm `device_node.hpp` has no `dma_buffer`

```bash
grep -n "dma_buffer" /home/nzh/MRNIU/SimpleKernel/src/device/include/device_node.hpp
```
Expected: **zero results**

### Step 3: Confirm `VirtioBlkDriver::Probe()` has no magic/device_id register reads

```bash
grep -n "kMmioMagicValue\|kDeviceId\|magic_reg\|device_id_reg" \
  /home/nzh/MRNIU/SimpleKernel/src/device/include/driver/virtio_blk_driver.hpp
```
Expected: results only appear inside `Match()`, not inside `Probe()`.

### Step 4: Confirm `Match()` exists in both drivers

```bash
grep -n "Match" \
  /home/nzh/MRNIU/SimpleKernel/src/device/include/driver/ns16550a_driver.hpp \
  /home/nzh/MRNIU/SimpleKernel/src/device/include/driver/virtio_blk_driver.hpp \
  /home/nzh/MRNIU/SimpleKernel/src/device/include/driver_registry.hpp \
  /home/nzh/MRNIU/SimpleKernel/src/device/include/device_manager.hpp
```
Expected: `Match` appears in all four files.

---

## Task 9: Commit

```bash
cd /home/nzh/MRNIU/SimpleKernel
git add src/device/include/driver_registry.hpp \
        src/device/include/device_manager.hpp \
        src/device/include/device_node.hpp \
        src/device/include/driver/ns16550a_driver.hpp \
        src/device/include/driver/virtio_blk_driver.hpp \
        src/device/device.cpp
git commit --signoff -m "refactor(device): separate hardware detection from driver initialization

Add Driver::Match() for fine-grained hardware detection. ProbeAll() now
calls match() before probe(). VirtioBlkDriver self-manages its DMA buffer;
DeviceNode::dma_buffer removed. DeviceInit() no longer contains any
driver-specific constants or strings."
```

Expected: Commit succeeds. Run `git log --oneline -1` to confirm.

---

## Success Criteria Checklist

- [ ] `DeviceInit()` contains no driver-specific strings or constants
- [ ] `VirtioBlkDriver::Probe()` contains no hardware register reads
- [ ] `DeviceNode` has no `dma_buffer` field
- [ ] Build passes: `cmake --preset build_riscv64 && make SimpleKernel`
- [ ] `grep "virtio,mmio" src/device/device.cpp` → zero results
- [ ] `grep "dma_buffer" src/device/include/device_node.hpp` → zero results
- [ ] `Match()` method present in `Ns16550aDriver` and `VirtioBlkDriver`
- [ ] `Driver` concept includes `Match()` requirement
- [ ] `DriverEntry` has `match` function pointer
- [ ] `ProbeAll()` calls `drv->match()` before `drv->probe()`
