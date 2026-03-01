# Design: Device Manager Split, FDT Refactor & Assert Hardening

**Date**: 2026-03-01
**Status**: Approved

---

## 背景

本次变更涵盖三个独立但相关的改进：

1. `device_manager.hpp` 头文件过长，可拆成声明与实现分离的结构
2. `PlatformBus` 在 FDT 枚举阶段会把中断控制器、时钟源等基础设施节点也纳入，导致 `ProbeAll` 大量打印 "no driver"；需通过 FDT 属性过滤在源头跳过这些节点，同时 `kernel_fdt.hpp` 随方法增加趋于复杂，一并重构
3. 构造函数与接口方法入参的 precondition 违反属于编程错误，应改用 `assert` 而非 `Expected` 错误码返回

---

## 变更 1：device_manager.hpp / device_manager.cpp 拆分

### 原则
- 模板方法 `RegisterBus<B>` 必须留在 `.hpp`（编译期实例化）
- 其余非模板方法移入 `.cpp`，`.hpp` 只保留声明

### 文件变动

| 文件 | 操作 | 内容 |
|------|------|------|
| `src/device/include/device_manager.hpp` | 修改 | 删除方法体，只保留声明；保留 `RegisterBus<B>` 模板完整实现；加 `#include <cassert>` |
| `src/device/device_manager.cpp` | 新建 | `ProbeAll()`、`FindDevice()`、`FindDevicesByType()` 实现；加 `#include <cassert>` |
| `src/device/CMakeLists.txt` | 修改 | `TARGET_SOURCES(device ...)` 中补充 `device_manager.cpp` |

### ProbeAll "no driver" 日志改动

删除逐设备 `klog::Debug("no driver for '%s'")` 行；在 `ProbeAll` 结尾改为摘要：

```cpp
klog::Info("DeviceManager: probed %zu device(s), %zu skipped (no driver)\n",
           probed, no_driver_count);
```

---

## 变更 2：kernel_fdt 重构 + ForEachDeviceNode

### 重构：kernel_fdt.hpp → .hpp + .cpp

`kernel_fdt.hpp` 目前约 715 行，全部是实现。只有两个模板方法（`ForEachNode`、`ForEachCompatibleNode`）因编译期实例化必须留在头文件；其余方法全部移入新文件 `src/kernel_fdt.cpp`。

| 文件 | 操作 | 内容 |
|------|------|------|
| `src/include/kernel_fdt.hpp` | 修改 | 只保留类声明、常量、模板方法，以及新增的 `ForEachDeviceNode` 模板；删除所有非模板方法体 |
| `src/kernel_fdt.cpp` | 新建 | 所有非模板方法的实现（构造函数除外，其初始化列表保留在 `.hpp`） |
| `src/CMakeLists.txt` | 修改 | `ADD_EXECUTABLE` 中补充 `kernel_fdt.cpp` |

**留在 `.hpp`（模板）：**
- `ForEachNode<Callback>`
- `ForEachCompatibleNode<Callback>`
- `ForEachDeviceNode<Callback>`（新增）

**移入 `.cpp`（非模板）：**
- `KernelFdt(uint64_t)` 构造函数
- `GetCoreCount()`、`CheckPSCI()`、`GetMemory()`、`GetSerial()`
- `GetTimebaseFrequency()`、`GetGIC()`、`GetGicDist()`、`GetGicCpu()`
- `GetAarch64Intid()`、`GetPlic()`
- 所有 `private` 辅助方法：`ValidateFdtHeader()`、`FindNode()`、`FindCompatibleNode()`、`FindEnabledCompatibleNode()`、`GetRegProperty()`、`CountNodesByDeviceType()`、`GetPsciMethod()`、`ValidatePsciFunctionIds()`

### ForEachDeviceNode — FDT 属性语义过滤

在 `kernel_fdt.hpp` 中新增模板方法，回调签名与 `ForEachNode` **完全相同**（对调用方透明）。在调用 callback 前，额外读取以下属性，命中任一则跳过该节点：

| FDT 属性 / device_type | 含义 | 典型节点 |
|---|---|---|
| `interrupt-controller` | 中断控制器 | PLIC、GIC、APIC |
| `#clock-cells` | 时钟提供者 | clk-fixed、cpg |
| `device_type = "cpu"` | CPU 节点 | /cpus/cpu@x |
| `device_type = "memory"` | 内存节点 | /memory@x |

实现草图：

```cpp
template <typename Callback>
[[nodiscard]] auto ForEachDeviceNode(Callback&& callback) const
    -> Expected<void> {
  assert(fdt_header_ != nullptr && "fdt_header_ is null");
  // 复用 ForEachNode 的遍历逻辑，在 offset 层面提前过滤
  int offset = -1, depth = 0;
  while (true) {
    offset = fdt_next_node(fdt_header_, offset, &depth);
    if (offset < 0) { /* break / error */ }

    // --- status 过滤（与 ForEachNode 相同）---
    // ... 检查 status != "okay"/"ok" 则 continue

    // --- 基础设施节点过滤（新增）---
    if (fdt_getprop(fdt_header_, offset, "interrupt-controller", nullptr))
      continue;
    if (fdt_getprop(fdt_header_, offset, "#clock-cells", nullptr))
      continue;
    {
      int len = 0;
      const auto* dt = fdt_getprop(fdt_header_, offset, "device_type", &len);
      if (dt) {
        const char* s = reinterpret_cast<const char*>(dt);
        if (strcmp(s, "cpu") == 0 || strcmp(s, "memory") == 0) continue;
      }
    }

    // --- 提取 compatible / mmio / irq，调用 callback ---
    // ... 与 ForEachNode 相同
  }
  return {};
}
```

`PlatformBus::Enumerate` 中：`fdt_.ForEachNode(...)` → `fdt_.ForEachDeviceNode(...)`

---

## 变更 3：assert 替换参数 precondition 检查

**约定**：使用 `assert(condition && "描述性消息")` 形式，与已有代码风格一致。

**只对编程错误用 assert；运行时资源耗尽仍用 `Expected`。**

### device_manager.cpp（新建文件中）

```cpp
auto DeviceManager::FindDevice(const char* name) -> Expected<DeviceNode*> {
  assert(name != nullptr && "FindDevice: name must not be null");
  // ...
}

auto DeviceManager::FindDevicesByType(DeviceType type, DeviceNode** out,
                                      size_t max) -> size_t {
  assert((out != nullptr || max == 0) &&
         "FindDevicesByType: out must not be null when max > 0");
  // ...
}
```

### driver_registry.hpp（mmio_helper::Prepare）

```cpp
// 删除：if (node.mmio_base == 0) return std::unexpected(...);
// 改为：
assert(node.mmio_base != 0 && "mmio_helper::Prepare: node has no MMIO base");
```

调用 `mmio_helper::Prepare` 的驱动（`Ns16550aDriver::Probe` 等）已在调用前通过 `DriverRegistry::FindDriver` 匹配了 compatible，有 MMIO base 是前提条件，属于编程错误而非运行时失败。

**`#include <cassert>` 新增位置：**
- `src/device/include/device_manager.hpp`
- `src/device/include/driver_registry.hpp`
- `src/device/device_manager.cpp`（新文件）

---

## 受影响文件清单

| 文件 | 操作 |
|------|------|
| `src/include/kernel_fdt.hpp` | 修改（重构 + ForEachDeviceNode） |
| `src/kernel_fdt.cpp` | **新建** |
| `src/CMakeLists.txt` | 修改（补 `kernel_fdt.cpp`） |
| `src/device/include/device_manager.hpp` | 修改（只留声明 + RegisterBus 模板 + cassert） |
| `src/device/device_manager.cpp` | **新建** |
| `src/device/CMakeLists.txt` | 修改（补 `device_manager.cpp`） |
| `src/device/include/driver_registry.hpp` | 修改（mmio_helper::Prepare 改 assert） |
| `src/device/include/platform_bus.hpp` | 修改（ForEachNode → ForEachDeviceNode） |

---

## 不在本次变更范围内

- `DriverRegistry::Register` / `FindDriver` 的 hpp→cpp 拆分（该文件本身未超出合理大小）
- 其他驱动文件的 assert 补全（单独跟进）
- `kernel_fdt.hpp` 中 assert 风格统一（已有的 assert 格式一致，不做修改）
