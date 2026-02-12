# Device 子系统

所有的设备应该模仿 Linux，提供 read/write/open/release/mmap/ioctl 等接口，由 DeviceManager 进行分发。

## 接口定义

核心接口位于 `include/device_operations.hpp`，基于 C++23 特性设计：

- **Deducing this** — CRTP-free 的静态多态，派生类无需显式传递自身类型作为模板参数（编译器自动推导）
- **std::expected** — 类型安全的错误处理，替代 errno 全局状态
- **std::span** — 零拷贝缓冲区传递

## 使用示例

```cpp
#include "device_operations.hpp"

class UartDevice : public DeviceOperations<UartDevice> {
 public:
  auto DoOpen(OpenFlags flags) -> Expected<void> {
    if (!flags.CanRead() && !flags.CanWrite()) {
      return std::unexpected(Error{ErrorCode::kInvalidArgument});
    }
    // 初始化 UART 硬件 ...
    return {};
  }

  auto DoRead(std::span<uint8_t> buffer, size_t offset) -> Expected<size_t> {
    // 从 UART FIFO 读取 ...
    return buffer.size();
  }

  auto DoWrite(std::span<const uint8_t> data, size_t offset) -> Expected<size_t> {
    // 写入 UART ...
    return data.size();
  }
};

// 调用方
UartDevice uart;
uart.OpenReadWrite();                        // 便利方法
auto result = uart.Read(buffer);             // offset 默认为 0
if (!result) {
  klog::Error("Read failed: %s\n", result.error().message());
}
uart.Release();
```

## 设计说明

- 所有 `DoXxx` 方法默认返回 `kDeviceNotSupported`，派生类只需覆写自己支持的操作
- 公共方法 (`Open/Read/Write/...`) 通过 deducing this 分派到派生类的 `DoXxx`
- 便利方法 (`ReadAll/WriteAll/OpenReadOnly/OpenReadWrite`) 组合基本操作，减少样板代码

## 设备类型层次

```
DeviceOperations<Derived>          ← 通用操作接口 (device_operations.hpp)
  ├── CharDevice<Derived>          ← 字符设备 (char_device.hpp)
  └── BlockDevice<Derived>         ← 块设备 (block_device.hpp)
```

### CharDevice — 字符设备

面向字节流，无随机访问。适用于 UART、键盘、RNG 等。

**相对基类的变化：**
- Read/Write 去掉 offset 参数（流设备无位置概念）
- 新增 `Poll(PollEvents)` — 非阻塞查询设备就绪状态
- 新增 `PutChar/GetChar` — 单字节操作便利方法

```cpp
class UartDevice : public CharDevice<UartDevice> {
 public:
  auto DoCharRead(std::span<uint8_t> buffer) -> Expected<size_t> { /* ... */ }
  auto DoCharWrite(std::span<const uint8_t> data) -> Expected<size_t> { /* ... */ }
  auto DoPoll(PollEvents requested) -> Expected<PollEvents> { /* ... */ }
};

UartDevice uart;
uart.PutChar('A');
auto ch = uart.GetChar();
```

### BlockDevice — 块设备

以固定大小扇区为最小读写单位，支持随机访问。适用于 virtio-blk、RAM disk、SD 卡等。

**相对基类的变化：**
- 新增 `ReadBlocks/WriteBlocks` — 以块为单位的批量读写
- 新增 `ReadBlock/WriteBlock` — 单块操作便利方法
- 新增 `Flush` — 刷写缓存到持久存储
- 新增 `GetBlockSize/GetBlockCount/GetCapacity` — 容量查询
- 基类的字节级 Read/Write 自动桥接到块操作（要求对齐）

```cpp
class VirtioBlkDevice : public BlockDevice<VirtioBlkDevice> {
 public:
  auto DoReadBlocks(uint64_t block_no, std::span<uint8_t> buffer,
                    size_t block_count) -> Expected<size_t> { /* ... */ }
  auto DoWriteBlocks(uint64_t block_no, std::span<const uint8_t> data,
                     size_t block_count) -> Expected<size_t> { /* ... */ }
  auto DoFlush() -> Expected<void> { return {}; }
  auto DoGetBlockSize() const -> size_t { return 512; }
  auto DoGetBlockCount() const -> uint64_t { return total_sectors_; }
};

VirtioBlkDevice disk;
disk.ReadBlock(0, sector_buf);            // 读第 0 扇区
disk.WriteBlocks(100, data, 8);           // 从第 100 扇区写 8 个块
auto cap = disk.GetCapacity();            // 总容量（字节）
disk.Flush();
```
