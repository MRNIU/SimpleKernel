# Device 模块重构实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 删除 `src/device/` 中约 1000 行死代码（`ops/` CRTP 层），将分散的 VirtIO 驱动整合为统一的 `VirtioDriver`，实现运行期设备类型自动检测。

**Architecture:** 用单一 `VirtioDriver`（匹配 `virtio,mmio` 和 `virtio,pci`）替换 `VirtioBlkDriver`。`VirtioDriver::Probe()` 内部读取 device_id 寄存器，自动分发到对应设备实现并注册到 `DeviceManager`。框架层 `DriverRegistry`/`ProbeAll()` 流程保持不变。

**Tech Stack:** C++23 freestanding, ETL (Embedded Template Library), GoogleTest (unit tests), CMake

**Design Doc:** `docs/plans/2026-03-01-device-refactor-design.md`

---

## 注意事项（阅读全文再开始）

- **所有编译验证**使用：`cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel -j$(nproc)`
- **单元测试**：`cd build_x86_64 && make unit-test`
- **include 路径**：device 模块的 include 根是 `src/device/include/`，所以 `#include "driver/foo.hpp"` 对应 `src/device/include/driver/foo.hpp`
- **header guard 格式**：`SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_FILENAME_HPP_`
- **版权头**：每个新文件第一行必须是 `/** @copyright Copyright The SimpleKernel Contributors */`
- **`etl::string_view` 禁用**：freestanding 不安全，用 `const char*`
- 本计划允许创建 `src/device/virtio_driver.cpp`（设计文档已批准的例外）

---

## Task 1: 移动 VirtioBlkBlockDevice 到独立头文件

**Files:**
- Create: `src/device/include/driver/detail/virtio/device/virtio_blk_vfs_adapter.hpp`
- Modify: `src/device/device.cpp`

**Step 1: 创建 `virtio_blk_vfs_adapter.hpp`**

内容是将 `device.cpp` 中 `namespace { class VirtioBlkBlockDevice ... }` 原样迁移过来，但改为具名 namespace 并加头文件 guard：

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_DEVICE_VIRTIO_BLK_VFS_ADAPTER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_DEVICE_VIRTIO_BLK_VFS_ADAPTER_HPP_

#include "block_device_provider.hpp"
#include "driver/detail/virtio/device/virtio_blk.hpp"
#include "expected.hpp"

namespace detail::virtio::blk {

/**
 * @brief Adapts VirtioBlk<> to vfs::BlockDevice.
 *
 * Wraps a VirtioBlk instance and forwards ReadSectors/WriteSectors
 * to the underlying VirtIO block device.
 */
class VirtioBlkVfsAdapter final : public vfs::BlockDevice {
 public:
  using VirtioBlkType = VirtioBlk<>;

  explicit VirtioBlkVfsAdapter(VirtioBlkType* dev) : dev_(dev) {}

  auto ReadSectors(uint64_t lba, uint32_t count, void* buf)
      -> Expected<size_t> override {
    auto* ptr = static_cast<uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto result = dev_->Read(lba + i, ptr + i * kSectorSize);
      if (!result) {
        return std::unexpected(Error(result.error().code));
      }
    }
    return static_cast<size_t>(count) * kSectorSize;
  }

  auto WriteSectors(uint64_t lba, uint32_t count, const void* buf)
      -> Expected<size_t> override {
    const auto* ptr = static_cast<const uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto result = dev_->Write(lba + i, ptr + i * kSectorSize);
      if (!result) {
        return std::unexpected(Error(result.error().code));
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

}  // namespace detail::virtio::blk

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_DEVICE_VIRTIO_BLK_VFS_ADAPTER_HPP_
```

**Step 2: 在 `device.cpp` 中添加 include，保留旧代码（暂不删除）**

在 `device.cpp` 顶部 include 列表中加入：
```cpp
#include "driver/detail/virtio/device/virtio_blk_vfs_adapter.hpp"
```

**Step 3: 编译验证**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel -j$(nproc)
```
期望：编译成功，无错误。

**Step 4: Commit**

```bash
git add src/device/include/driver/detail/virtio/device/virtio_blk_vfs_adapter.hpp \
        src/device/device.cpp
git commit -m "refactor(device): extract VirtioBlkVfsAdapter to dedicated header

Move VirtioBlkBlockDevice class (renamed VirtioBlkVfsAdapter) out of
device.cpp into its own header. No functional change.

Signed-off-by: $(git config user.name) <$(git config user.email)>"
```

---

## Task 2: 为 VirtioDriver 写测试（先写失败的测试）

**Files:**
- Create: `tests/unit_test/virtio_driver_test.cpp`
- Modify: `tests/unit_test/CMakeLists.txt`

**Step 1: 了解测试可覆盖的范围**

VirtIO probe 涉及真实 MMIO 硬件，无法在 host 单元测试中执行完整流程。
可测试的部分：
1. `VirtioDriver::GetEntry()` 元数据正确（name、match_table）
2. `VirtioDriver::MatchStatic()` 在 mmio_base=0 时返回 false（无硬件依赖的快速路径）

**Step 2: 创建测试文件**

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#include <gtest/gtest.h>

// 测试目标在 Task 3 中创建，现在这个测试应当编译失败（找不到头文件）
#include "driver/virtio_driver.hpp"

TEST(VirtioDriverTest, GetEntryNameIsVirtio) {
  const auto& entry = VirtioDriver::GetEntry();
  EXPECT_STREQ(entry.name, "virtio");
}

TEST(VirtioDriverTest, MatchStaticReturnsFalseWhenNoMmioBase) {
  DeviceNode node{};
  node.mmio_base = 0;
  EXPECT_FALSE(VirtioDriver::MatchStatic(node));
}

TEST(VirtioDriverTest, MatchTableContainsVirtioMmio) {
  const auto& entry = VirtioDriver::GetEntry();
  bool found = false;
  for (const auto& m : entry.match_table) {
    if (m.bus == BusType::kPlatform &&
        __builtin_strcmp(m.compatible, "virtio,mmio") == 0) {
      found = true;
    }
  }
  EXPECT_TRUE(found);
}
```

**Step 3: 在 `tests/unit_test/CMakeLists.txt` 中添加测试文件**

在 `ADD_EXECUTABLE` 的源文件列表中加入：
```cmake
    virtio_driver_test.cpp
```

同时在 `TARGET_SOURCES` 中加入：
```cmake
    ${CMAKE_SOURCE_DIR}/src/device/virtio_driver.cpp
```

**Step 4: 确认测试失败（因为 virtio_driver.hpp 还不存在）**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test 2>&1 | head -20
```
期望：编译错误，`driver/virtio_driver.hpp: No such file or directory`

**Step 5: Commit**

```bash
git add tests/unit_test/virtio_driver_test.cpp tests/unit_test/CMakeLists.txt
git commit -m "test(device): add failing tests for VirtioDriver

Tests expect VirtioDriver::GetEntry() and MatchStatic() which will be
implemented in the next task (TDD).

Signed-off-by: $(git config user.name) <$(git config user.email)>"
```

---

## Task 3: 创建 `virtio_driver.hpp` 和 `virtio_driver.cpp`

**Files:**
- Create: `src/device/include/driver/virtio_driver.hpp`
- Create: `src/device/virtio_driver.cpp`

**Step 1: 创建 `virtio_driver.hpp`**

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_DRIVER_HPP_

#include <etl/io_port.h>
#include <etl/optional.h>
#include <etl/span.h>
#include <variant>

#include "device_manager.hpp"
#include "device_node.hpp"
#include "driver/detail/virtio/device/virtio_blk.hpp"
#include "driver/detail/virtio/device/virtio_blk_vfs_adapter.hpp"
#include "driver/detail/virtio/traits.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "io_buffer.hpp"
#include "kernel_log.hpp"
#include "kstd_memory"
#include "mmio_helper.hpp"

/**
 * @brief 统一 VirtIO 驱动
 *
 * 匹配所有 virtio,mmio 和 virtio,pci 兼容设备。
 * Probe() 运行期读取 device_id 寄存器，自动分发到对应设备实现
 * 并注册到 DeviceManager。调用方无需关心具体 VirtIO 设备类型。
 *
 * @see docs/plans/2026-03-01-device-refactor-design.md
 */
class VirtioDriver {
 public:
  /// VirtIO 设备类型枚举（来自 VirtIO 1.2 规范）
  enum class DeviceId : uint32_t {
    kNet = 1,
    kBlock = 2,
    kConsole = 3,
    kEntropy = 4,
    kGpu = 16,
    kInput = 18,
  };

  static constexpr uint32_t kMmioRegionSize = 0x1000;
  static constexpr uint32_t kDefaultQueueCount = 1;
  static constexpr uint32_t kDefaultQueueSize = 128;
  static constexpr size_t kMinDmaBufferSize = 32768;

  // --- Registration API ---

  static auto Instance() -> VirtioDriver& {
    static VirtioDriver inst;
    return inst;
  }

  /**
   * @brief 返回驱动注册入口
   *
   * 匹配 virtio,mmio 兼容字符串；MatchStatic 检查 VirtIO magic number。
   */
  static auto GetEntry() -> const DriverEntry& {
    static const DriverEntry entry{
        .name = "virtio",
        .match_table = etl::span<const MatchEntry>(kMatchTable),
        .match = etl::delegate<bool(
            DeviceNode&)>::create<&VirtioDriver::MatchStatic>(),
        .probe = etl::delegate<Expected<void>(DeviceNode&)>::create<
            VirtioDriver, &VirtioDriver::Probe>(Instance()),
        .remove = etl::delegate<Expected<void>(DeviceNode&)>::create<
            VirtioDriver, &VirtioDriver::Remove>(Instance()),
    };
    return entry;
  }

  // --- Driver lifecycle ---

  /**
   * @brief 硬件检测：验证 VirtIO magic number
   *
   * 只检查 magic number（0x74726976），不检查 device_id——
   * device_id 的分发在 Probe() 中完成。
   *
   * @pre  node.mmio_base != 0
   * @return true 如果设备响应 VirtIO magic
   */
  static auto MatchStatic(DeviceNode& node) -> bool;

  /**
   * @brief 初始化 VirtIO 设备
   *
   * 运行期读取 device_id，分发到对应设备实现，创建并注册设备。
   *
   * @pre  node.mmio_base != 0，MatchStatic() 已返回 true
   * @post 设备已注册到 DeviceManager（通过 block_device_provider 等）
   */
  auto Probe(DeviceNode& node) -> Expected<void>;

  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    blk_device_.reset();
    dma_buffer_.reset();
    return {};
  }

  [[nodiscard]] auto GetBlkDevice()
      -> detail::virtio::blk::VirtioBlk<>* {
    return blk_device_.has_value() ? &blk_device_.value() : nullptr;
  }

 private:
  static constexpr MatchEntry kMatchTable[] = {
      {BusType::kPlatform, "virtio,mmio"},
  };

  etl::optional<detail::virtio::blk::VirtioBlk<>> blk_device_;
  etl::unique_ptr<IoBuffer> dma_buffer_;
  uint32_t irq_{0};
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_DRIVER_HPP_
```

**Step 2: 创建 `virtio_driver.cpp`**

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#include "driver/virtio_driver.hpp"

#include <etl/io_port.h>

#include "driver/detail/virtio/transport/mmio.hpp"
#include "kernel_error.h"
#include "kernel_log.hpp"
#include "mmio_helper.hpp"

auto VirtioDriver::MatchStatic(DeviceNode& node) -> bool {
  if (node.mmio_base == 0) return false;

  auto ctx = mmio_helper::Prepare(node, kMmioRegionSize);
  if (!ctx) return false;

  etl::io_port_ro<uint32_t> magic_reg{reinterpret_cast<void*>(ctx->base)};
  if (magic_reg.read() != detail::virtio::kMmioMagicValue) {
    klog::Debug("VirtioDriver: 0x%lX not a VirtIO device\n", ctx->base);
    return false;
  }
  return true;
}

auto VirtioDriver::Probe(DeviceNode& node) -> Expected<void> {
  auto ctx = mmio_helper::Prepare(node, kMmioRegionSize);
  if (!ctx) {
    return std::unexpected(ctx.error());
  }

  // 读取 device_id
  etl::io_port_ro<uint32_t> device_id_reg{reinterpret_cast<void*>(
      ctx->base + detail::virtio::MmioTransport::MmioReg::kDeviceId)};
  const auto device_id = static_cast<DeviceId>(device_id_reg.read());

  switch (device_id) {
    case DeviceId::kBlock: {
      // 分配 DMA buffer
      dma_buffer_ = kstd::make_unique<IoBuffer>(kMinDmaBufferSize);
      if (!dma_buffer_ || !dma_buffer_->IsValid() ||
          dma_buffer_->GetBuffer().size() < kMinDmaBufferSize) {
        klog::Err("VirtioDriver: failed to allocate DMA buffer at 0x%lX\n",
                  ctx->base);
        return std::unexpected(Error(ErrorCode::kOutOfMemory));
      }

      uint64_t extra_features =
          static_cast<uint64_t>(
              detail::virtio::blk::BlkFeatureBit::kSegMax) |
          static_cast<uint64_t>(
              detail::virtio::blk::BlkFeatureBit::kSizeMax) |
          static_cast<uint64_t>(
              detail::virtio::blk::BlkFeatureBit::kBlkSize) |
          static_cast<uint64_t>(
              detail::virtio::blk::BlkFeatureBit::kFlush) |
          static_cast<uint64_t>(
              detail::virtio::blk::BlkFeatureBit::kGeometry);

      auto result = detail::virtio::blk::VirtioBlk<>::Create(
          ctx->base, dma_buffer_->GetBuffer().data(), kDefaultQueueCount,
          kDefaultQueueSize, extra_features);
      if (!result.has_value()) {
        klog::Err("VirtioDriver: VirtioBlk Create failed at 0x%lX\n",
                  ctx->base);
        return std::unexpected(Error(result.error().code));
      }

      blk_device_.emplace(std::move(*result));
      node.type = DeviceType::kBlock;
      irq_ = node.irq;

      klog::Info(
          "VirtioDriver: block device at 0x%lX, capacity=%lu sectors, "
          "irq=%u\n",
          ctx->base, blk_device_.value().GetCapacity(), irq_);
      return {};
    }

    default:
      klog::Warn("VirtioDriver: unsupported device_id=%u at 0x%lX\n",
                 static_cast<uint32_t>(device_id), ctx->base);
      return std::unexpected(Error(ErrorCode::kNotSupported));
  }
}
```

**Step 3: 运行测试，确认通过**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```
期望：3 个 VirtioDriver 测试全部 PASS。

**Step 4: Commit**

```bash
git add src/device/include/driver/virtio_driver.hpp \
        src/device/virtio_driver.cpp
git commit -m "feat(device): add unified VirtioDriver with runtime device-type dispatch

Replace VirtioBlkDriver with VirtioDriver that auto-detects VirtIO
device type (blk/net/gpu/...) at runtime by reading device_id register.
Caller (DeviceInit) no longer needs to know about VirtIO device types.

Signed-off-by: $(git config user.name) <$(git config user.email)>"
```

---

## Task 4: 更新 CMakeLists.txt，添加 virtio_driver.cpp

**Files:**
- Modify: `src/device/CMakeLists.txt`

**Step 1: 在 `src/device/CMakeLists.txt` 中添加 `virtio_driver.cpp`**

当前内容：
```cmake
ADD_LIBRARY (device INTERFACE)
TARGET_INCLUDE_DIRECTORIES (device BEFORE
                            INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)
TARGET_SOURCES (device INTERFACE device.cpp)
```

修改 `TARGET_SOURCES` 行：
```cmake
TARGET_SOURCES (device INTERFACE device.cpp virtio_driver.cpp)
```

**Step 2: 编译验证**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel -j$(nproc)
```
期望：编译成功。

**Step 3: Commit**

```bash
git add src/device/CMakeLists.txt
git commit -m "build(device): add virtio_driver.cpp to device library sources

Signed-off-by: $(git config user.name) <$(git config user.email)>"
```

---

## Task 5: 更新 `device.cpp` 使用 VirtioDriver

**Files:**
- Modify: `src/device/device.cpp`

**Step 1: 替换 `device.cpp` 的内容**

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#include "block_device_provider.hpp"
#include "device_manager.hpp"
#include "driver/ns16550a_driver.hpp"
#include "driver/virtio_driver.hpp"
#include "kernel.h"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "platform_bus.hpp"

/// Device subsystem initialisation entry point
auto DeviceInit() -> void {
  DeviceManagerSingleton::create();
  auto& dm = DeviceManagerSingleton::instance();

  if (auto r = dm.GetRegistry().Register(Ns16550aDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register Ns16550aDriver failed: %s\n",
              r.error().message());
    return;
  }

  if (auto r = dm.GetRegistry().Register(VirtioDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register VirtioDriver failed: %s\n",
              r.error().message());
    return;
  }

  PlatformBus platform_bus(KernelFdtSingleton::instance());
  if (auto r = dm.RegisterBus(platform_bus); !r) {
    klog::Err("DeviceInit: PlatformBus enumeration failed: %s\n",
              r.error().message());
    return;
  }

  if (auto r = dm.ProbeAll(); !r) {
    klog::Err("DeviceInit: ProbeAll failed: %s\n", r.error().message());
    return;
  }

  klog::Info("DeviceInit: complete\n");
}

auto GetVirtioBlkBlockDevice() -> vfs::BlockDevice* {
  auto* raw = VirtioDriver::Instance().GetBlkDevice();
  if (raw == nullptr) {
    klog::Err("GetVirtioBlkBlockDevice: no virtio-blk device probed\n");
    return nullptr;
  }
  static detail::virtio::blk::VirtioBlkVfsAdapter adapter(raw);
  return &adapter;
}
```

**Step 2: 编译验证**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel -j$(nproc)
```
期望：编译成功，无警告。

**Step 3: 运行测试确认无回归**

```bash
cd build_x86_64 && make unit-test
```
期望：所有测试 PASS。

**Step 4: Commit**

```bash
git add src/device/device.cpp
git commit -m "refactor(device): update DeviceInit to use unified VirtioDriver

Replace VirtioBlkDriver::GetEntry() with VirtioDriver::GetEntry().
DeviceInit no longer mentions specific VirtIO device types.

Signed-off-by: $(git config user.name) <$(git config user.email)>"
```

---

## Task 6: 修复 `ns16550a_driver.hpp` 的 include

**Files:**
- Modify: `src/device/include/driver/ns16550a_driver.hpp`

**Step 1: 将 `ns16550a_driver.hpp` 第 10 行的 include 从 `ns16550a_device.hpp` 改为 `ns16550a.hpp`**

将：
```cpp
#include "driver/detail/ns16550a/ns16550a_device.hpp"
```
改为：
```cpp
#include "driver/detail/ns16550a/ns16550a.hpp"
```

（`Ns16550aDriver::Ns16550aType` 是 `detail::ns16550a::Ns16550a`，不是 `Ns16550aDevice`，直接包含 `ns16550a.hpp` 即可）

**Step 2: 编译验证**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel -j$(nproc)
```

**Step 3: Commit**

```bash
git add src/device/include/driver/ns16550a_driver.hpp
git commit -m "refactor(device): ns16550a_driver include ns16550a.hpp directly

Remove dependency on ns16550a_device.hpp (CRTP wrapper, to be deleted).

Signed-off-by: $(git config user.name) <$(git config user.email)>"
```

---

## Task 7: 修复 `pl011_driver.hpp` 的 include

**Files:**
- Modify: `src/device/include/driver/pl011_driver.hpp`

**Step 1: 查看 `pl011_driver.hpp` 当前内容**

当前文件只有：
```cpp
#include "driver/detail/pl011/pl011_device.hpp"
namespace pl011 {
using namespace detail::pl011;  // NOLINT(google-build-using-namespace)
}
```

确认 `pl011_device.hpp` 里有什么（查看 `detail/pl011/pl011_device.hpp` 的 namespace），然后改为直接 include `pl011.hpp`：

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_PL011_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_PL011_DRIVER_HPP_

#include "driver/detail/pl011/pl011.hpp"

namespace pl011 {
using namespace detail::pl011;  // NOLINT(google-build-using-namespace)
}  // namespace pl011

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_PL011_DRIVER_HPP_
```

注意：如果 `pl011_device.hpp` 中有 `pl011_driver.hpp` 需要的额外类型，保留对应 include。

**Step 2: 编译验证**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel -j$(nproc)
```

**Step 3: Commit**

```bash
git add src/device/include/driver/pl011_driver.hpp
git commit -m "refactor(device): pl011_driver include pl011.hpp directly

Signed-off-by: $(git config user.name) <$(git config user.email)>"
```

---

## Task 8: 删除所有死代码文件

**Files to delete:**

```
src/device/include/driver/ops/device_ops_base.hpp
src/device/include/driver/ops/char_device.hpp
src/device/include/driver/ops/block_device_ops.hpp
src/device/include/driver/detail/uart_device.hpp
src/device/include/driver/detail/ns16550a/ns16550a_device.hpp
src/device/include/driver/detail/pl011/pl011_device.hpp
src/device/include/driver/detail/virtio/device/virtio_blk_device.hpp
src/device/include/driver/virtio_blk_driver.hpp
```

**Step 1: 删除文件**

```bash
rm src/device/include/driver/ops/device_ops_base.hpp \
   src/device/include/driver/ops/char_device.hpp \
   src/device/include/driver/ops/block_device_ops.hpp \
   src/device/include/driver/detail/uart_device.hpp \
   src/device/include/driver/detail/ns16550a/ns16550a_device.hpp \
   src/device/include/driver/detail/pl011/pl011_device.hpp \
   src/device/include/driver/detail/virtio/device/virtio_blk_device.hpp \
   src/device/include/driver/virtio_blk_driver.hpp
rmdir src/device/include/driver/ops/  # 目录应该为空了
```

**Step 2: 检查是否还有文件 include 了这些被删除的文件**

```bash
grep -r "ops/device_ops_base\|ops/char_device\|ops/block_device_ops\|uart_device\|ns16550a_device\|pl011_device\|virtio_blk_device\|virtio_blk_driver" \
     src/ tests/ --include="*.hpp" --include="*.cpp" --include="*.h"
```

期望：无任何输出（所有引用已在 Task 5/6/7 中消除）。

**Step 3: 编译验证**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel -j$(nproc)
```
期望：编译成功。

**Step 4: 运行所有测试**

```bash
cd build_x86_64 && make unit-test
```
期望：全部 PASS。

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor(device): delete dead ops/ CRTP layer and unused device wrappers

Remove ~1000 lines of dead code:
- driver/ops/ (device_ops_base.hpp, char_device.hpp, block_device_ops.hpp)
- driver/detail/uart_device.hpp
- driver/detail/ns16550a/ns16550a_device.hpp
- driver/detail/pl011/pl011_device.hpp
- driver/detail/virtio/device/virtio_blk_device.hpp
- driver/virtio_blk_driver.hpp (replaced by virtio_driver.hpp)

Signed-off-by: $(git config user.name) <$(git config user.email)>"
```

---

## Task 9: 更新 AGENTS.md

**Files:**
- Modify: `src/device/AGENTS.md`

**Step 1: 更新 AGENTS.md 以反映新结构**

需要修改以下内容：
1. STRUCTURE 部分：移除 `virtio_blk_driver.hpp`，添加 `virtio_driver.hpp`
2. WHERE TO LOOK 部分：更新驱动示例
3. CONVENTIONS 部分：将 "Entirely header-only" 修改为允许 `virtio_driver.cpp` 的例外
4. 删除对 `ops/` 层的任何提及
5. 添加对新 VirtIO 设备扩展步骤的说明

关键改动——CONVENTIONS 中：
```markdown
- **Mostly header-only** — `device.cpp` and `virtio_driver.cpp` are the only .cpp files.
  Do NOT create additional .cpp files without strong justification.
```

以及添加扩展指引：
```markdown
- **Adding a new VirtIO device** → Add `detail/virtio/device/virtio_xxx.hpp`,
  add a `case DeviceId::kXxx` to `virtio_driver.cpp`, no changes to DeviceInit needed.
```

**Step 2: 编译并运行测试确认无回归**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

**Step 3: Commit**

```bash
git add src/device/AGENTS.md
git commit -m "docs(device): update AGENTS.md for new VirtioDriver architecture

- Document virtio_driver.cpp as the second .cpp exception
- Add VirtIO device extension guide (add device_id case only)
- Remove references to deleted ops/ layer

Signed-off-by: $(git config user.name) <$(git config user.email)>"
```

---

## Task 10: 最终验证

**Step 1: 全量编译所有架构（若环境支持）**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel -j$(nproc)
```

**Step 2: 运行完整测试套件**

```bash
cd build_x86_64 && make unit-test
```
期望：全部测试 PASS，零失败。

**Step 3: 确认死代码已彻底删除**

```bash
# 确认 ops/ 目录不存在
ls src/device/include/driver/ops/ 2>&1

# 确认已删文件不被引用
grep -r "virtio_blk_driver\|ops/char_device\|ops/block_device\|ns16550a_device\|pl011_device\|virtio_blk_device\|uart_device" \
     src/ tests/ --include="*.hpp" --include="*.cpp" --include="*.h"
```
期望：第一条命令报 "No such file"，第二条无输出。

**Step 4: 统计删减行数**

```bash
git diff HEAD~8 --stat
```

**Step 5: 最终 commit（若有未提交的小改动）**

```bash
git status
# 若干净则无需 commit
```

---

## 完成标准

- [ ] 编译通过（x86_64 + 其他可用架构）
- [ ] 所有单元测试 PASS
- [ ] `src/device/include/driver/ops/` 目录不存在
- [ ] `grep -r "VirtioBlkDriver" src/` 无输出
- [ ] `VirtioDriver::GetEntry()` 替代了所有 VirtioBlkDriver 注册
- [ ] `device.cpp` 中无内嵌类定义
- [ ] `AGENTS.md` 已更新
