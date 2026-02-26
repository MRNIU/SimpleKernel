# VirtIO-Blk × FatFS 集成设计

**日期**: 2026-02-27
**状态**: 已确认，待实现
**范围**: 将 `VirtioBlkDriver` 包装为 `vfs::BlockDevice`，挂载 FatFS 到 `/mnt/fat`

---

## 一、目标

在内核启动序列中，通过 `VirtioBlkDriver` 访问 QEMU `virtio-blk-device` 挂载的 `rootfs.img`（FAT32 格式），并将其以 FatFS 文件系统的形式挂载到 VFS 路径 `/mnt/fat`，使上层代码可通过 `vfs::Open / Read / Write` 等标准接口读写文件。

---

## 二、现状

| 组件 | 状态 |
|------|------|
| QEMU virtio-blk 配置 | ✅ 已有（`CMakePresets.json` 三架构均配置 `-device virtio-blk-device,drive=hd0`） |
| `VirtioBlkDriver<Traits>::Probe()` | ✅ 已实现，探测成功后 `GetDevice()` 返回 `VirtioBlk<Traits>*` |
| `DriverRegistry::GetDriverInstance<D>()` | ✅ 已有，返回驱动静态单例 |
| `vfs::BlockDevice` 纯虚接口 | ✅ 已有（`ReadSectors / WriteSectors / GetSectorSize / GetSectorCount / GetName`） |
| `FatFsFileSystem::Mount(vfs::BlockDevice*)` | ✅ 已有，`diskio.cpp` 已对接 `vfs::BlockDevice` |
| `FileSystemInit()` 中挂载 ramfs | ✅ 已有 |
| **`VirtioBlkBlockDevice` 适配器** | ❌ 缺失 |
| **`GetVirtioBlkBlockDevice()` 隔离函数** | ❌ 缺失 |
| **FatFS 挂载到 `/mnt/fat`** | ❌ 缺失 |
| **`rootfs.img` FAT32 格式化** | ❌ 当前为全零内容 |

---

## 三、架构设计

### 3.1 调用链

```
FileSystemInit()
├── vfs::Init()                           [已有]
├── Mount ramfs to "/"                    [已有]
│
├── GetVirtioBlkBlockDevice()             [新建：device 层暴露]
│     └── DriverRegistry::GetDriverInstance<VirtioBlkDriver<PlatformTraits>>()
│               .GetDevice()              → VirtioBlk<PlatformTraits>*
│         VirtioBlkBlockDevice adapter(dev)  [file-scope static]
│         return &adapter                 → vfs::BlockDevice*
│
├── FatFsFileSystem::Mount(block_device)  [FatFS 挂载]
└── vfs::GetMountTable().Mount("/mnt/fat", &fat_fs, block_device)
```

### 3.2 职责分离

- **`VirtioBlkBlockDevice`**（在 `device.cpp` 文件作用域内，不对外暴露类型）
  纯接口适配：将 `VirtioBlk<Traits>::Read(lba, buf)` / `Write(lba, buf)` 转换为 `vfs::BlockDevice` 接口。

- **`GetVirtioBlkBlockDevice()`**（声明在 `block_device_provider.hpp`，实现在 `device.cpp`）
  隔离 `filesystem` 层对 `virtio_blk_driver.hpp` 的直接依赖，是唯一的跨层接口。

- **`FileSystemInit()`**（`filesystem.cpp`）
  调用 `GetVirtioBlkBlockDevice()`，创建 `FatFsFileSystem` 实例，挂载到 `/mnt/fat`。

- **`cmake/functions.cmake`**
  生成 `rootfs.img` 后立即 `mkfs.fat -F 32` 格式化，保证挂载时 FAT 结构有效。

---

## 四、新增文件与变更

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `src/device/include/block_device_provider.hpp` | 新建 | 声明 `GetVirtioBlkBlockDevice() -> vfs::BlockDevice*` |
| `src/device/device.cpp` | 修改 | 新增 `VirtioBlkBlockDevice` 适配器类（文件作用域）及 `GetVirtioBlkBlockDevice()` 实现 |
| `src/filesystem/filesystem.cpp` | 修改 | 挂载 FatFS 到 `/mnt/fat` |
| `src/filesystem/CMakeLists.txt` | 修改 | 为 filesystem 目标添加 `block_device_provider.hpp` 所在目录的 include 路径 |
| `cmake/functions.cmake` | 修改 | `rootfs.img` 生成后追加 `mkfs.fat -F 32` |

---

## 五、详细设计

### 5.1 `block_device_provider.hpp`

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 块设备提供者接口
 */
#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_PROVIDER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_PROVIDER_HPP_

#include "block_device.hpp"  // vfs::BlockDevice

/**
 * @brief 返回已探测的 virtio-blk 设备的 vfs::BlockDevice 接口
 * @pre   DeviceInit() 及 ProbeAll() 已完成
 * @return vfs::BlockDevice* 成功时返回适配器指针；探测未完成时返回 nullptr
 */
auto GetVirtioBlkBlockDevice() -> vfs::BlockDevice*;

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_PROVIDER_HPP_ */
```

### 5.2 `device.cpp` 新增部分

```cpp
// --- 文件作用域（不对外暴露类型） ---

namespace {

class VirtioBlkBlockDevice final : public vfs::BlockDevice {
 public:
  using VirtioBlkType = VirtioBlkDriver<PlatformTraits>::VirtioBlkType;

  explicit VirtioBlkBlockDevice(VirtioBlkType* dev) : dev_(dev) {}

  [[nodiscard]] auto ReadSectors(uint64_t lba, uint32_t count, void* buf)
      -> Expected<size_t> override {
    auto* ptr = static_cast<uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto r = dev_->Read(lba + i, ptr + i * kSectorSize);
      if (!r) return std::unexpected(df_bridge::ToKernelError(r.error()));
    }
    return static_cast<size_t>(count) * kSectorSize;
  }

  [[nodiscard]] auto WriteSectors(uint64_t lba, uint32_t count,
                                  const void* buf) -> Expected<size_t> override {
    const auto* ptr = static_cast<const uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto r = dev_->Write(lba + i, ptr + i * kSectorSize);
      if (!r) return std::unexpected(df_bridge::ToKernelError(r.error()));
    }
    return static_cast<size_t>(count) * kSectorSize;
  }

  [[nodiscard]] auto GetSectorSize() const -> uint32_t override { return kSectorSize; }
  [[nodiscard]] auto GetSectorCount() const -> uint64_t override {
    return dev_->GetCapacity();
  }
  [[nodiscard]] auto GetName() const -> const char* override { return "virtio-blk0"; }

 private:
  static constexpr uint32_t kSectorSize = 512;
  VirtioBlkType* dev_;
};

}  // namespace

auto GetVirtioBlkBlockDevice() -> vfs::BlockDevice* {
  using Driver = VirtioBlkDriver<PlatformTraits>;
  auto* raw = DriverRegistry::GetDriverInstance<Driver>().GetDevice();
  if (raw == nullptr) {
    klog::Err("GetVirtioBlkBlockDevice: virtio-blk not probed\n");
    return nullptr;
  }
  static VirtioBlkBlockDevice adapter(raw);
  return &adapter;
}
```

### 5.3 `filesystem.cpp` 新增部分

```cpp
#include "block_device_provider.hpp"
#include "fatfs.hpp"

// 在 FileSystemInit() 末尾追加：
auto* blk = GetVirtioBlkBlockDevice();
if (blk != nullptr) {
  static fatfs::FatFsFileSystem fat_fs(0);
  auto fat_mount = fat_fs.Mount(blk);
  if (!fat_mount.has_value()) {
    klog::Err("FileSystemInit: FatFS mount failed: %s\n",
              fat_mount.error().message());
  } else {
    auto vfs_mount = vfs::GetMountTable().Mount("/mnt/fat", &fat_fs, blk);
    if (!vfs_mount.has_value()) {
      klog::Err("FileSystemInit: VFS mount /mnt/fat failed: %s\n",
                vfs_mount.error().message());
    } else {
      klog::Info("FileSystemInit: FatFS mounted at /mnt/fat\n");
    }
  }
}
```

### 5.4 `cmake/functions.cmake` 变更

```cmake
# 在 dd 命令之后追加 mkfs.fat
ADD_CUSTOM_COMMAND(
  ...
  COMMAND dd if=/dev/zero of=${ROOTFS_IMG} bs=1M count=64
  COMMAND mkfs.fat -F 32 ${ROOTFS_IMG}    # <-- 新增
  ...
)
```

---

## 六、约束与注意事项

1. **`VirtioBlk::Read/Write` 签名**：参数为 `(uint64_t lba, void* buf)` 返回 `device_framework::Expected<void>`。需通过 `df_bridge::ToKernelError` 转换错误类型。
2. **静态适配器实例**：`VirtioBlkBlockDevice adapter(raw)` 为函数内 `static`，生命周期持续到内核退出，无需堆分配。
3. **`FatFsFileSystem` 实例**：同样为 `FileSystemInit()` 内 `static`，避免堆分配。
4. **构建依赖**：`filesystem` CMake 目标需要能够找到 `block_device_provider.hpp`（位于 `src/device/include/`）。
5. **mkfs.fat 工具**：需要宿主机安装 `dosfstools`（Docker 镜像 `ptrnull233/simple_kernel:latest` 已包含）。
6. **`/mnt/fat` 目录**：VFS `Mount` 接口直接接受路径字符串，无需预先创建目录节点。

---

## 七、验证标准

启动日志中应出现：

```
DeviceManager: 'virtio-blk@...' bound to 'virtio-blk'
VirtioBlkDriver: block device at 0x..., capacity=131072 sectors, irq=...
FileSystemInit: FatFS mounted at /mnt/fat
FileSystemInit: complete
```

无 `FatFS mount failed` 或 `VFS mount /mnt/fat failed` 错误。
