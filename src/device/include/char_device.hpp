/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_CHAR_DEVICE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_CHAR_DEVICE_HPP_

#include "device_operations.hpp"

/**
 * @brief Poll 事件标志位
 *
 * 类似 Linux 的 POLLIN / POLLOUT 等，用于 Poll() 查询设备就绪状态。
 */
struct PollEvents {
  uint32_t value;

  /// 有数据可读
  static constexpr uint32_t kIn = 1U << 0;
  /// 可以写入（不会阻塞）
  static constexpr uint32_t kOut = 1U << 1;
  /// 发生错误
  static constexpr uint32_t kErr = 1U << 2;
  /// 挂起（对端关闭）
  static constexpr uint32_t kHup = 1U << 3;

  constexpr explicit PollEvents(uint32_t v = 0) : value(v) {}
  constexpr auto operator|(PollEvents other) const -> PollEvents {
    return PollEvents{value | other.value};
  }
  constexpr auto operator&(PollEvents other) const -> PollEvents {
    return PollEvents{value & other.value};
  }
  constexpr explicit operator bool() const { return value != 0; }

  [[nodiscard]] constexpr auto HasIn() const -> bool {
    return (value & kIn) != 0;
  }
  [[nodiscard]] constexpr auto HasOut() const -> bool {
    return (value & kOut) != 0;
  }
  [[nodiscard]] constexpr auto HasErr() const -> bool {
    return (value & kErr) != 0;
  }
};

/**
 * @brief 字符设备抽象接口
 *
 * 面向字节流的设备，不支持随机访问。
 * Read/Write 无 offset 参数，新增 Poll 和 PutChar/GetChar。
 *
 * @tparam Derived 具体字符设备类型
 *
 * @pre  派生类至少实现 DoCharRead 或 DoCharWrite 之一
 */
template <class Derived>
class CharDevice : public DeviceOperations<Derived> {
 public:
  /**
   * @brief 从字符设备读取数据（无 offset）
   *
   * @param  buffer  目标缓冲区
   * @return Expected<size_t> 实际读取的字节数
   */
  auto Read(this Derived& self, std::span<uint8_t> buffer) -> Expected<size_t> {
    return self.DoCharRead(buffer);
  }

  /**
   * @brief 向字符设备写入数据（无 offset）
   *
   * @param  data  待写入数据
   * @return Expected<size_t> 实际写入的字节数
   */
  auto Write(this Derived& self, std::span<const uint8_t> data)
      -> Expected<size_t> {
    return self.DoCharWrite(data);
  }

  /**
   * @brief 查询设备就绪状态（非阻塞）
   *
   * @param  requested  感兴趣的事件集合
   * @return Expected<PollEvents> 当前就绪的事件集合
   */
  auto Poll(this Derived& self, PollEvents requested) -> Expected<PollEvents> {
    return self.DoPoll(requested);
  }

  /**
   * @brief 写入单个字节
   */
  auto PutChar(this Derived& self, uint8_t ch) -> Expected<void> {
    std::span<const uint8_t> data{&ch, 1};
    auto result = self.Write(data);
    if (!result) {
      return std::unexpected(result.error());
    }
    return {};
  }

  /**
   * @brief 读取单个字节
   */
  auto GetChar(this Derived& self) -> Expected<uint8_t> {
    uint8_t ch = 0;
    std::span<uint8_t> buffer{&ch, 1};
    auto result = self.Read(buffer);
    if (!result) {
      return std::unexpected(result.error());
    }
    if (*result == 0) {
      return std::unexpected(Error{ErrorCode::kDeviceReadFailed});
    }
    return ch;
  }

 protected:
  /**
   * @brief 字符设备 Read 实现（派生类覆写）
   */
  auto DoCharRead([[maybe_unused]] std::span<uint8_t> buffer)
      -> Expected<size_t> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief 字符设备 Write 实现（派生类覆写）
   */
  auto DoCharWrite([[maybe_unused]] std::span<const uint8_t> data)
      -> Expected<size_t> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief 字符设备 Poll 实现（派生类覆写）
   */
  auto DoPoll([[maybe_unused]] PollEvents requested) -> Expected<PollEvents> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  auto DoRead(std::span<uint8_t> buffer, [[maybe_unused]] size_t offset)
      -> Expected<size_t> {
    return static_cast<Derived*>(this)->DoCharRead(buffer);
  }

  auto DoWrite(std::span<const uint8_t> data, [[maybe_unused]] size_t offset)
      -> Expected<size_t> {
    return static_cast<Derived*>(this)->DoCharWrite(data);
  }

  ~CharDevice() = default;
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_CHAR_DEVICE_HPP_ */
