# Device Manager Split + FDT Refactor + Assert Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 拆分 `device_manager.hpp`、重构 `kernel_fdt.hpp`（拆分 + 新增 `ForEachDeviceNode`）、将入参 precondition 改为 `assert`，消除 `ProbeAll` 大量 "no driver" 噪声日志。

**Architecture:** 三个独立批次，每批次独立编译验证后提交。`kernel_fdt` 重构体量最大（~500 行迁移），单独作为一个批次。`device_manager` 拆分和 assert 合并为第二批次。`PlatformBus` 切换 + `ProbeAll` 日志改为最后一批次。

**Tech Stack:** C++23 freestanding, libfdt, CMake, clang-format (pre-commit), QEMU 验证

---

## Task 1：kernel_fdt 重构——新建 kernel_fdt.cpp，迁移非模板方法

**Files:**
- Modify: `src/include/kernel_fdt.hpp`
- Create: `src/kernel_fdt.cpp`
- Modify: `src/CMakeLists.txt`

### Step 1：新建 `src/kernel_fdt.cpp`，迁移所有非模板方法体

将以下方法的**完整实现**从 `kernel_fdt.hpp` 剪切，原样粘贴到 `kernel_fdt.cpp`：

- 构造函数 `KernelFdt(uint64_t header)`
- `GetCoreCount()`、`CheckPSCI()`、`GetMemory()`、`GetSerial()`
- `GetTimebaseFrequency()`、`GetGIC()`、`GetGicDist()`、`GetGicCpu()`
- `GetAarch64Intid(const char*)`、`GetPlic()`
- 所有 `private` helper：`ValidateFdtHeader()`、`FindNode()`、`FindCompatibleNode()`、`FindEnabledCompatibleNode()`、`GetRegProperty()`、`CountNodesByDeviceType()`、`GetPsciMethod()`、`ValidatePsciFunctionIds()`

`kernel_fdt.cpp` 文件头：

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

// 禁用 GCC/Clang 的警告
#include <libfdt_env.h>
#ifdef __GNUC__
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wpedantic"
#endif

#include <libfdt.h>

#ifdef __GNUC__
#pragma GCC diagnostic pop
#endif

#include <cpu_io.h>

#include <array>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <tuple>
#include <utility>

#include "expected.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "kstd_cstring"
```

### Step 2：修改 `kernel_fdt.hpp`——保留声明和模板方法

在 `.hpp` 中：

1. 删除所有已移入 `.cpp` 的方法体，改为纯声明（加 `;`）
2. 构造函数 `KernelFdt(uint64_t header)` 改为纯声明：
   ```cpp
   explicit KernelFdt(uint64_t header);
   ```
3. `private` helper 方法改为纯声明
4. **保留完整实现**的方法（模板）：`ForEachNode`、`ForEachCompatibleNode`
5. 静态常量 `kPsciCpuOnFuncId` 等保留在 `.hpp`（`private static constexpr`）

### Step 3：`src/CMakeLists.txt` 补充 kernel_fdt.cpp

找到：
```cmake
ADD_EXECUTABLE (${PROJECT_NAME} main.cpp io_buffer.cpp syscall.cpp)
```

改为：
```cmake
ADD_EXECUTABLE (${PROJECT_NAME} main.cpp io_buffer.cpp syscall.cpp
                                kernel_fdt.cpp)
```

### Step 4：编译验证

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

预期：`0 errors`，链接成功。

### Step 5：格式检查

```bash
cd /home/nzh/MRNIU/SimpleKernel
pre-commit run --files src/include/kernel_fdt.hpp src/kernel_fdt.cpp src/CMakeLists.txt
```

预期：所有检查 Passed。

### Step 6：提交

```bash
git add src/include/kernel_fdt.hpp src/kernel_fdt.cpp src/CMakeLists.txt
git commit --signoff -m "refactor(fdt): split kernel_fdt.hpp into hpp/cpp"
```

---

## Task 2：kernel_fdt 新增 ForEachDeviceNode

**Files:**
- Modify: `src/include/kernel_fdt.hpp`

### Step 1：在 `kernel_fdt.hpp` 中 `ForEachCompatibleNode` 之后插入 `ForEachDeviceNode`

```cpp
/**
 * @brief 遍历 FDT 中所有"叶设备"节点，自动跳过基础设施节点。
 *
 * 在 ForEachNode 的基础上，额外过滤掉以下节点：
 *   - 具有 `interrupt-controller` 属性（中断控制器）
 *   - 具有 `#clock-cells` 属性（时钟提供者）
 *   - `device_type = "cpu"` 或 `device_type = "memory"`
 *
 * @tparam Callback 签名与 ForEachNode 完全相同：
 *   bool(const char* node_name, const char* compatible_data,
 *        size_t compatible_len, uint64_t mmio_base, size_t mmio_size,
 *        uint32_t irq)
 * @param  callback 节点处理函数，返回 false 停止遍历
 * @return Expected<void>
 * @pre    fdt_header_ 不为空
 */
template <typename Callback>
[[nodiscard]] auto ForEachDeviceNode(Callback&& callback) const
    -> Expected<void> {
  assert(fdt_header_ != nullptr && "ForEachDeviceNode: fdt_header_ is null");

  int offset = -1;
  int depth = 0;

  while (true) {
    offset = fdt_next_node(fdt_header_, offset, &depth);
    if (offset < 0) {
      if (offset == -FDT_ERR_NOTFOUND) {
        break;
      }
      return std::unexpected(Error(ErrorCode::kFdtParseFailed));
    }

    const char* node_name = fdt_get_name(fdt_header_, offset, nullptr);
    if (node_name == nullptr) {
      continue;
    }

    // status 过滤（与 ForEachNode 相同）
    int status_len = 0;
    const auto* status_prop =
        fdt_get_property(fdt_header_, offset, "status", &status_len);
    if (status_prop != nullptr) {
      const char* status = reinterpret_cast<const char*>(status_prop->data);
      if (strcmp(status, "okay") != 0 && strcmp(status, "ok") != 0) {
        continue;
      }
    }

    // 基础设施节点过滤
    if (fdt_getprop(fdt_header_, offset, "interrupt-controller", nullptr) !=
        nullptr) {
      continue;
    }
    if (fdt_getprop(fdt_header_, offset, "#clock-cells", nullptr) != nullptr) {
      continue;
    }
    {
      int dt_len = 0;
      const auto* dt_prop =
          fdt_get_property(fdt_header_, offset, "device_type", &dt_len);
      if (dt_prop != nullptr) {
        const char* dt = reinterpret_cast<const char*>(dt_prop->data);
        if (strcmp(dt, "cpu") == 0 || strcmp(dt, "memory") == 0) {
          continue;
        }
      }
    }

    // compatible 提取
    const char* compatible_data = nullptr;
    size_t compatible_len = 0;
    int compat_len = 0;
    const auto* compat_prop =
        fdt_get_property(fdt_header_, offset, "compatible", &compat_len);
    if (compat_prop != nullptr && compat_len > 0) {
      compatible_data = reinterpret_cast<const char*>(compat_prop->data);
      compatible_len = static_cast<size_t>(compat_len);
    }

    // reg 提取
    uint64_t mmio_base = 0;
    size_t mmio_size = 0;
    int reg_len = 0;
    const auto* reg_prop =
        fdt_get_property(fdt_header_, offset, "reg", &reg_len);
    if (reg_prop != nullptr &&
        static_cast<size_t>(reg_len) >= 2 * sizeof(uint64_t)) {
      const auto* reg = reinterpret_cast<const uint64_t*>(reg_prop->data);
      mmio_base = fdt64_to_cpu(reg[0]);
      mmio_size = fdt64_to_cpu(reg[1]);
    }

    // interrupts 提取
    uint32_t irq = 0;
    int irq_len = 0;
    const auto* irq_prop =
        fdt_get_property(fdt_header_, offset, "interrupts", &irq_len);
    if (irq_prop != nullptr &&
        static_cast<size_t>(irq_len) >= sizeof(uint32_t)) {
      const auto* interrupts =
          reinterpret_cast<const uint32_t*>(irq_prop->data);
      irq = fdt32_to_cpu(interrupts[0]);
    }

    if (!callback(node_name, compatible_data, compatible_len, mmio_base,
                  mmio_size, irq)) {
      break;
    }
  }

  return {};
}
```

### Step 2：编译验证

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

预期：`0 errors`。

### Step 3：格式检查并提交

```bash
cd /home/nzh/MRNIU/SimpleKernel
pre-commit run --files src/include/kernel_fdt.hpp
git add src/include/kernel_fdt.hpp
git commit --signoff -m "feat(fdt): add ForEachDeviceNode to filter infrastructure nodes"
```

---

## Task 3：device_manager.hpp/cpp 拆分 + assert 加固

**Files:**
- Modify: `src/device/include/device_manager.hpp`
- Create: `src/device/device_manager.cpp`
- Modify: `src/device/CMakeLists.txt`

### Step 1：新建 `src/device/device_manager.cpp`

文件头：

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#include <cassert>

#include "device_manager.hpp"
#include "kernel_log.hpp"
#include "kstd_cstring"
```

将以下三个方法从 `.hpp` 剪切，粘贴到 `.cpp`，并加入 assert：

```cpp
auto DeviceManager::ProbeAll() -> Expected<void> {
  LockGuard guard(lock_);

  size_t probed = 0;
  size_t no_driver_count = 0;
  for (size_t i = 0; i < device_count_; ++i) {
    auto& node = devices_[i];

    const auto* drv = registry_.FindDriver(node);
    if (drv == nullptr) {
      ++no_driver_count;
      continue;
    }

    if (!drv->match(node)) {
      klog::Debug("DeviceManager: driver '%s' rejected '%s'\n", drv->name,
                  node.name);
      continue;
    }

    if (node.bound) continue;
    node.bound = true;

    klog::Info("DeviceManager: probing '%s' with driver '%s'\n", node.name,
               drv->name);

    auto result = drv->probe(node);
    if (!result.has_value()) {
      klog::Err("DeviceManager: probe '%s' failed: %s\n", node.name,
                result.error().message());
      node.bound = false;
      continue;
    }

    ++probed;
    klog::Info("DeviceManager: '%s' bound to '%s'\n", node.name, drv->name);
  }

  klog::Info("DeviceManager: probed %zu device(s), %zu skipped (no driver)\n",
             probed, no_driver_count);
  return {};
}

auto DeviceManager::FindDevice(const char* name) -> Expected<DeviceNode*> {
  assert(name != nullptr && "FindDevice: name must not be null");
  for (size_t i = 0; i < device_count_; ++i) {
    if (kstd::strcmp(devices_[i].name, name) == 0) {
      return &devices_[i];
    }
  }
  return std::unexpected(Error(ErrorCode::kDeviceNotFound));
}

auto DeviceManager::FindDevicesByType(DeviceType type, DeviceNode** out,
                                      size_t max) -> size_t {
  assert((out != nullptr || max == 0) &&
         "FindDevicesByType: out must not be null when max > 0");
  size_t found = 0;
  for (size_t i = 0; i < device_count_ && found < max; ++i) {
    if (devices_[i].type == type) {
      out[found++] = &devices_[i];
    }
  }
  return found;
}
```

### Step 2：修改 `device_manager.hpp`

1. 在 include 区加 `#include <cassert>`
2. 将 `ProbeAll()`、`FindDevice()`、`FindDevicesByType()` 的方法体删除，改为纯声明：

```cpp
auto ProbeAll() -> Expected<void>;
auto FindDevice(const char* name) -> Expected<DeviceNode*>;
auto FindDevicesByType(DeviceType type, DeviceNode** out, size_t max)
    -> size_t;
```

3. `RegisterBus<B>` 模板、`GetRegistry()` inline getter 保持不变

### Step 3：`src/device/CMakeLists.txt` 补充 device_manager.cpp

找到：
```cmake
TARGET_SOURCES (device INTERFACE device.cpp)
```

改为：
```cmake
TARGET_SOURCES (device INTERFACE device.cpp device_manager.cpp)
```

### Step 4：编译验证

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

预期：`0 errors`。

### Step 5：格式检查并提交

```bash
cd /home/nzh/MRNIU/SimpleKernel
pre-commit run --files src/device/include/device_manager.hpp \
               src/device/device_manager.cpp \
               src/device/CMakeLists.txt
git add src/device/include/device_manager.hpp \
        src/device/device_manager.cpp \
        src/device/CMakeLists.txt
git commit --signoff -m "refactor(device): split device_manager into hpp/cpp, add assert on params"
```

---

## Task 4：mmio_helper::Prepare 改 assert + PlatformBus 切换 ForEachDeviceNode

**Files:**
- Modify: `src/device/include/driver_registry.hpp`
- Modify: `src/device/include/platform_bus.hpp`

### Step 1：`driver_registry.hpp` — mmio_helper::Prepare 改 assert

找到：

```cpp
[[nodiscard]] inline auto Prepare(const DeviceNode& node, size_t default_size)
    -> Expected<ProbeContext> {
  if (node.mmio_base == 0) {
    klog::Err("mmio_helper: no MMIO base for '%s'\n", node.name);
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }
```

改为：

```cpp
[[nodiscard]] inline auto Prepare(const DeviceNode& node, size_t default_size)
    -> Expected<ProbeContext> {
  assert(node.mmio_base != 0 &&
         "mmio_helper::Prepare: node has no MMIO base; driver matched wrong node");
```

同时在 `driver_registry.hpp` includes 区加 `#include <cassert>`（如尚未存在）。

### Step 2：`platform_bus.hpp` — ForEachNode → ForEachDeviceNode

找到（约第 32 行）：

```cpp
auto result = fdt_.ForEachNode(
```

改为：

```cpp
auto result = fdt_.ForEachDeviceNode(
```

仅此一处改动，callback 体不需要任何修改（签名完全相同）。

### Step 3：编译验证

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

预期：`0 errors`。

### Step 4：格式检查并提交

```bash
cd /home/nzh/MRNIU/SimpleKernel
pre-commit run --files src/device/include/driver_registry.hpp \
               src/device/include/platform_bus.hpp
git add src/device/include/driver_registry.hpp \
        src/device/include/platform_bus.hpp
git commit --signoff -m "fix(device): filter infra FDT nodes, assert mmio_base precondition"
```

---

## Task 5：整体验证

### Step 1：完整构建并运行

```bash
cd build_riscv64 && make SimpleKernel && make run
```

观察 QEMU 串口日志，确认：
- `PlatformBus: found '...'` 行不再出现中断控制器（plic、gic、clint 等）
- `DeviceManager: probed X device(s), Y skipped (no driver)` Y 值明显减少（或为 0）
- `DeviceInit: complete` 正常出现

### Step 2：pre-commit 全量检查

```bash
cd /home/nzh/MRNIU/SimpleKernel
pre-commit run --all-files
```

预期：所有检查 Passed。

### Step 3：设计文档提交

```bash
git add doc/plans/2026-03-01-device-fdt-refactor-design.md \
        doc/plans/2026-03-01-device-fdt-refactor-plan.md
git commit --signoff -m "docs: add device-fdt refactor design and implementation plan"
```
