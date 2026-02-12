/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_OPERATIONS_BASE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_OPERATIONS_BASE_HPP_

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <expected>
#include <span>

#include "expected.hpp"

/**
 * @brief 设备打开模式标志位
 */
struct OpenFlags {
  uint32_t value;

  static constexpr uint32_t kRead = 1U << 0;
  static constexpr uint32_t kWrite = 1U << 1;
  static constexpr uint32_t kReadWrite = kRead | kWrite;
  static constexpr uint32_t kAppend = 1U << 2;
  static constexpr uint32_t kCreate = 1U << 3;
  static constexpr uint32_t kTruncate = 1U << 4;
  static constexpr uint32_t kNonBlock = 1U << 5;
  static constexpr uint32_t kExclusive = 1U << 6;

  constexpr explicit OpenFlags(uint32_t v = 0) : value(v) {}
  constexpr auto operator|(OpenFlags other) const -> OpenFlags {
    return OpenFlags{value | other.value};
  }
  constexpr auto operator&(OpenFlags other) const -> OpenFlags {
    return OpenFlags{value & other.value};
  }
  constexpr explicit operator bool() const { return value != 0; }

  [[nodiscard]] constexpr auto CanRead() const -> bool {
    return (value & kRead) != 0;
  }
  [[nodiscard]] constexpr auto CanWrite() const -> bool {
    return (value & kWrite) != 0;
  }
};

/**
 * @brief mmap 操作的内存保护标志
 */
struct ProtFlags {
  uint32_t value;

  static constexpr uint32_t kNone = 0;
  static constexpr uint32_t kRead = 1U << 0;
  static constexpr uint32_t kWrite = 1U << 1;
  static constexpr uint32_t kExec = 1U << 2;

  constexpr explicit ProtFlags(uint32_t v = 0) : value(v) {}
  constexpr auto operator|(ProtFlags other) const -> ProtFlags {
    return ProtFlags{value | other.value};
  }
  constexpr auto operator&(ProtFlags other) const -> ProtFlags {
    return ProtFlags{value & other.value};
  }
};

/**
 * @brief mmap 操作的映射标志
 */
struct MapFlags {
  uint32_t value;

  static constexpr uint32_t kShared = 1U << 0;
  static constexpr uint32_t kPrivate = 1U << 1;
  static constexpr uint32_t kFixed = 1U << 2;
  static constexpr uint32_t kAnonymous = 1U << 3;

  constexpr explicit MapFlags(uint32_t v = 0) : value(v) {}
  constexpr auto operator|(MapFlags other) const -> MapFlags {
    return MapFlags{value | other.value};
  }
  constexpr auto operator&(MapFlags other) const -> MapFlags {
    return MapFlags{value & other.value};
  }
};

/**
 * @brief 设备操作抽象接口
 *
 * 派生类只需 override 支持的 DoXxx 方法，未覆写的操作默认返回
 * kDeviceNotSupported。
 *
 * @tparam Derived 具体设备类型
 */
template <class Derived>
class DeviceOperationsBase {
 public:
  /**
   * @brief 打开设备
   *
   * @param  flags  打开模式，使用 OpenFlags 组合
   * @return Expected<void>
   */
  auto Open(this Derived& self, OpenFlags flags) -> Expected<void> {
    bool expected = false;
    if (!self.opened_.compare_exchange_strong(expected, true)) {
      return std::unexpected(Error{ErrorCode::kDeviceAlreadyOpen});
    }
    auto result = self.DoOpen(flags);
    if (!result) {
      self.opened_.store(false);
    }
    return result;
  }

  /**
   * @brief 释放（关闭）设备
   */
  auto Release(this Derived& self) -> Expected<void> {
    if (!self.opened_.load()) {
      return std::unexpected(Error{ErrorCode::kDeviceNotOpen});
    }
    auto result = self.DoRelease();
    if (result) {
      self.opened_.store(false);
    }
    return result;
  }

  /**
   * @brief 从设备读取数据
   *
   * @param  buffer  目标缓冲区
   * @param  offset  设备内的偏移量（字节）
   * @return Expected<size_t> 实际读取的字节数
   *
   * @pre  buffer.data() != nullptr && !buffer.empty()
   * @post 返回值 <= buffer.size()
   */
  auto Read(this Derived& self, std::span<uint8_t> buffer, size_t offset = 0)
      -> Expected<size_t> {
    if (!self.opened_.load()) {
      return std::unexpected(Error{ErrorCode::kDeviceNotOpen});
    }
    return self.DoRead(buffer, offset);
  }

  /**
   * @brief 向设备写入数据
   *
   * @param  data    待写入的数据缓冲区
   * @param  offset  设备内的偏移量（字节）
   * @return Expected<size_t> 实际写入的字节数
   *
   * @pre  data.data() != nullptr && !data.empty()
   * @post 返回值 <= data.size()
   */
  auto Write(this Derived& self, std::span<const uint8_t> data,
             size_t offset = 0) -> Expected<size_t> {
    if (!self.opened_.load()) {
      return std::unexpected(Error{ErrorCode::kDeviceNotOpen});
    }
    return self.DoWrite(data, offset);
  }

  /**
   * @brief 将设备内存映射到进程地址空间
   *
   * @param  addr    建议映射的虚拟地址（0 表示由内核选择）
   * @param  length  映射长度（字节）
   * @param  prot    内存保护标志
   * @param  flags   映射标志
   * @param  offset  设备内的偏移量（字节）
   * @return Expected<uintptr_t> 实际映射的虚拟地址
   *
   * @pre  length > 0
   * @pre  offset 按页对齐
   */
  auto Mmap(this Derived& self, uintptr_t addr, size_t length, ProtFlags prot,
            MapFlags flags, size_t offset) -> Expected<uintptr_t> {
    if (!self.opened_.load()) {
      return std::unexpected(Error{ErrorCode::kDeviceNotOpen});
    }
    return self.DoMmap(addr, length, prot, flags, offset);
  }

  /**
   * @brief 设备特定的控制操作（ioctl）
   *
   * @param  request  控制命令码
   * @param  arg      命令参数
   * @return Expected<int64_t> 命令结果
   */
  auto Ioctl(this Derived& self, uint32_t request, uintptr_t arg = 0)
      -> Expected<int64_t> {
    if (!self.opened_.load()) {
      return std::unexpected(Error{ErrorCode::kDeviceNotOpen});
    }
    return self.DoIoctl(request, arg);
  }

  /**
   * @brief 从设备偏移 0 处读取全部缓冲区
   */
  auto ReadAll(this Derived& self, std::span<uint8_t> buffer)
      -> Expected<size_t> {
    return self.Read(buffer, 0);
  }

  /**
   * @brief 从设备偏移 0 处写入全部数据
   */
  auto WriteAll(this Derived& self, std::span<const uint8_t> data)
      -> Expected<size_t> {
    return self.Write(data, 0);
  }

  /**
   * @brief 以只读模式打开设备
   */
  auto OpenReadOnly(this Derived& self) -> Expected<void> {
    return self.Open(OpenFlags{OpenFlags::kRead});
  }

  /**
   * @brief 以读写模式打开设备
   */
  auto OpenReadWrite(this Derived& self) -> Expected<void> {
    return self.Open(OpenFlags{OpenFlags::kReadWrite});
  }

 protected:
  /// @brief 查询设备是否已打开
  [[nodiscard]] auto IsOpened() const -> bool { return opened_.load(); }

  /// @brief 默认 Open 实现（返回 kDeviceNotSupported）
  auto DoOpen([[maybe_unused]] OpenFlags flags) -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief 默认 Release 实现
   */
  auto DoRelease() -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief 默认 Read 实现
   */
  auto DoRead([[maybe_unused]] std::span<uint8_t> buffer,
              [[maybe_unused]] size_t offset) -> Expected<size_t> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief 默认 Write 实现
   */
  auto DoWrite([[maybe_unused]] std::span<const uint8_t> data,
               [[maybe_unused]] size_t offset) -> Expected<size_t> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief 默认 Mmap 实现
   */
  auto DoMmap([[maybe_unused]] uintptr_t addr, [[maybe_unused]] size_t length,
              [[maybe_unused]] ProtFlags prot, [[maybe_unused]] MapFlags flags,
              [[maybe_unused]] size_t offset) -> Expected<uintptr_t> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief 默认 Ioctl 实现
   */
  auto DoIoctl([[maybe_unused]] uint32_t request,
               [[maybe_unused]] uintptr_t arg) -> Expected<int64_t> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /// @name 构造/析构函数
  /// @{
  DeviceOperationsBase() = default;
  ~DeviceOperationsBase() = default;
  DeviceOperationsBase(const DeviceOperationsBase&) = delete;
  auto operator=(const DeviceOperationsBase&) -> DeviceOperationsBase& = delete;
  DeviceOperationsBase(DeviceOperationsBase&& other) noexcept
      : opened_(other.opened_.load()) {
    other.opened_.store(false);
  }
  auto operator=(DeviceOperationsBase&& other) noexcept
      -> DeviceOperationsBase& {
    if (this != &other) {
      opened_.store(other.opened_.load());
      other.opened_.store(false);
    }
    return *this;
  }
  /// @}

 private:
  /// @brief 设备打开状态（原子操作，支持多核并发）
  std::atomic<bool> opened_{false};
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_OPERATIONS_BASE_HPP_ */
