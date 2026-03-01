# Device 模块重构设计文档

**日期**：2026-03-01
**状态**：已批准，待实现
**范围**：`src/device/`

---

## 背景与动机

`src/device/` 模块存在三个主要复杂点：

1. **`ops/` CRTP 层是死代码**：`device_ops_base.hpp`、`char_device.hpp`、`block_device_ops.hpp` 约 745 行，但实际驱动（`Ns16550aDriver`、`VirtioBlkDriver`）均绕过此层直接使用 `detail::` 实现，`*_device.hpp` CRTP 包装同样未被消费。

2. **`device.cpp` 职责混杂**：`VirtioBlkBlockDevice`（VFS 适配器类）内嵌在 `DeviceInit()` 初始化文件中。

3. **VirtIO 驱动接口设计不足**：`VirtioBlkDriver` 暴露 transport / virtqueue 模板参数，且不同 VirtIO 设备类型（blk/net/gpu 等）的判断分散在调用方。

---

## 设计目标

- 净减约 1000 行死代码
- `src/device/` 公共接口对调用方隐藏 VirtIO 内部细节（transport 类型、设备类型）
- 保留 `detail/virtio/` 的分层结构，支持未来扩展更多 VirtIO 设备
- 允许为 VirtIO probe 逻辑创建 `.cpp` 实现文件（有意覆盖 AGENTS.md "header-only" 约束）

---

## 选定方案：删除死层 + 统一 VirtIO Probe 接口

### 一、删除的文件（死代码，~1000 行）

| 文件 | 原因 |
|------|------|
| `src/device/include/driver/ops/device_ops_base.hpp` | CRTP 基类，无驱动继承 |
| `src/device/include/driver/ops/char_device.hpp` | 同上 |
| `src/device/include/driver/ops/block_device_ops.hpp` | 同上 |
| `src/device/include/driver/detail/uart_device.hpp` | 依赖 ops/，随之删除 |
| `src/device/include/driver/detail/ns16550a/ns16550a_device.hpp` | CRTP 包装，驱动绕过它 |
| `src/device/include/driver/detail/pl011/pl011_device.hpp` | 同上 |
| `src/device/include/driver/detail/virtio/device/virtio_blk_device.hpp` | 同上，由新 virtio_driver 替代 |
| `src/device/include/driver/virtio_blk_driver.hpp` | 被 virtio_driver.hpp 替代 |

### 二、新增的文件

| 文件 | 内容 |
|------|------|
| `src/device/include/driver/virtio_driver.hpp` | `virtio::Probe()` 声明、`DeviceId` 枚举、transport 类型前向声明 |
| `src/device/virtio_driver.cpp` | `Probe()` 实现、`DetectTransport()`、`CreateAndRegisterBlk()` 等 |
| `src/device/include/driver/detail/virtio/device/virtio_blk_vfs_adapter.hpp` | 从 `device.cpp` 移入的 VFS 适配器类 |

### 三、修改的文件

| 文件 | 改动 |
|------|------|
| `src/device/device.cpp` | 移除内嵌 `VirtioBlkBlockDevice` 类，改用 `virtio::Probe(node, mgr)` |
| `src/device/include/driver/ns16550a_driver.hpp` | 移除对 `ns16550a_device.hpp` 的 include |
| `src/device/include/driver/pl011_driver.hpp` | 移除对 `pl011_device.hpp` 的 include |
| `src/device/AGENTS.md` | 更新说明：允许 `virtio_driver.cpp`，移除 header-only 对 virtio probe 的约束 |

### 四、保持不变

- `detail/virtio/transport/`（mmio.hpp、pci.hpp、transport.hpp）
- `detail/virtio/virt_queue/`（misc.hpp、split.hpp、virtqueue_base.hpp）
- `detail/virtio/device/virtio_blk.hpp`（模板实现，仍在头文件）
- 框架层：`device_manager.hpp`、`driver_registry.hpp`、`platform_bus.hpp`、`bus.hpp`、`device_node.hpp`

---

## 目录结构（After）

```
src/device/
├── device.cpp                     ← 只有 DeviceInit()，无内嵌类
├── virtio_driver.cpp              ← 新增：Probe 实现
└── include/
    ├── bus.hpp
    ├── block_device_provider.hpp
    ├── device_manager.hpp
    ├── device_node.hpp
    ├── driver_registry.hpp
    ├── mmio_helper.hpp
    ├── platform_bus.hpp
    └── driver/
        ├── acpi_driver.hpp
        ├── ns16550a_driver.hpp
        ├── pl011_driver.hpp
        ├── traits.hpp
        ├── virtio_driver.hpp      ← 新增（替代 virtio_blk_driver.hpp）
        └── detail/
            ├── mmio_accessor.hpp
            ├── acpi/acpi.hpp
            ├── ns16550a/
            │   └── ns16550a.hpp
            ├── pl011/
            │   └── pl011.hpp
            └── virtio/
                ├── defs.h
                ├── platform_config.hpp
                ├── traits.hpp
                ├── device/
                │   ├── device_initializer.hpp
                │   ├── virtio_blk.hpp
                │   ├── virtio_blk_defs.h
                │   ├── virtio_blk_vfs_adapter.hpp  ← 新增（从 device.cpp 移入）
                │   └── virtio_*.h（占位）
                ├── transport/
                │   ├── mmio.hpp
                │   ├── pci.hpp
                │   └── transport.hpp
                └── virt_queue/
                    ├── misc.hpp
                    ├── split.hpp
                    └── virtqueue_base.hpp
```

---

## virtio::Probe 设计

### 公共接口

```cpp
// driver/virtio_driver.hpp
namespace virtio {

enum class DeviceId : uint32_t {
  kNet     = 1,
  kBlock   = 2,
  kConsole = 3,
  kGpu     = 16,
  kInput   = 18,
};

// 唯一公共入口：运行期自动判断 transport 类型 + 设备类型，注册到 DeviceManager
auto Probe(const DeviceNode& node, DeviceManager& mgr) -> Expected<void>;

}  // namespace virtio
```

### 数据流

```
virtio::Probe(node, mgr)
  │
  ├─ 1. DetectTransport(node)
  │       node 有 MMIO reg → MmioTransport(base, size)
  │       node 有 PCI BAR  → PciTransport(bar)
  │       失败              → Unexpected(Error::kInvalidArg)
  │
  ├─ 2. std::visit → transport.ReadDeviceId()
  │       读 VirtIO device_id 寄存器
  │
  ├─ 3. switch(device_id)
  │       case kBlock   → CreateAndRegisterBlk(transport, mgr)
  │       case kNet     → CreateAndRegisterNet(transport, mgr)  [future]
  │       default       → Unexpected(Error::kUnsupported)
  │
  └─ return Expected<void>
```

### 内部 transport 类型

```cpp
// virtio_driver.cpp (内部，对外不可见)
using AnyTransport = std::variant<MmioTransport, PciTransport>;
```

### DeviceInit 调用方式（After）

```cpp
// device.cpp
for (auto& node : platform_bus.FindCompatible("virtio,mmio")) {
    if (auto r = virtio::Probe(node, device_manager); !r) {
        klog::Warn() << "VirtIO probe failed: " << r.error();
        // 不中止启动，继续枚举其他设备
    }
}
```

---

## 错误处理

| 错误点 | 错误类型 | 处理方式 |
|--------|---------|---------|
| DeviceNode 无 MMIO 也无 PCI 资源 | `Error::kInvalidArg` | `DetectTransport` 返回 `Unexpected` |
| transport 初始化失败 | `Error::kIo` | `ReadDeviceId` 返回 `Unexpected` |
| device_id 无对应驱动 | `Error::kUnsupported` | `Probe` 返回 `Unexpected`，DeviceInit 记录警告继续 |
| `VirtioBlk::Create` 失败 | `Error::kIo` | `CreateAndRegisterBlk` 返回 `Unexpected` |

---

## 测试策略

**新增测试文件**：`tests/device/virtio_probe_test.cpp`

| 测试用例 | 验证点 |
|---------|--------|
| `DetectTransport_Mmio` | MMIO DeviceNode → 返回 `MmioTransport` |
| `DetectTransport_Pci` | PCI DeviceNode → 返回 `PciTransport` |
| `DetectTransport_NoResource` | 空 DeviceNode → 返回 `Error::kInvalidArg` |
| `Probe_BlockDevice` | device_id=2 → `CreateAndRegisterBlk` 被调用 |
| `Probe_UnknownDevice` | device_id=99 → 返回 `Error::kUnsupported` |
| `Probe_TransportFail` | transport 初始化失败 → 错误正确传播 |

Mock 方式：实现 `MockTransport` 满足与 `MmioTransport`/`PciTransport` 相同的接口 concept，注入模板参数。

---

## 扩展新 VirtIO 设备的步骤（未来）

1. 在 `detail/virtio/device/` 下新增 `virtio_net.hpp`
2. 在 `virtio_driver.cpp` 的 `switch` 中加 `case DeviceId::kNet`
3. 调用方 `device.cpp` **零改动**

---

## 备注

- 本设计有意覆盖 `src/device/AGENTS.md` 中"Entirely header-only except device.cpp"的约束，`virtio_driver.cpp` 是第二个合法的 `.cpp` 文件。需同步更新 AGENTS.md。
- `std::variant` 在本项目中可用（用户确认）。
