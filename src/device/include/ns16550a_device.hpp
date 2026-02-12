/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DRIVER_NS16550A_INCLUDE_NS16550A_HPP_
#define SIMPLEKERNEL_SRC_DRIVER_NS16550A_INCLUDE_NS16550A_HPP_

#include <atomic>
#include <cstdint>
#include <span>

#include "driver/ns16550a/include/ns16550a.h"
#include "expected.hpp"
#include "operations/char_device_operations.hpp"

class Ns16550aDevice : public CharDevice<Ns16550aDevice> {
 public:
  Ns16550aDevice() = default;
  explicit Ns16550aDevice(uint64_t base_addr) : driver_(base_addr) {}

  /// 直接访问底层驱动（用于中断处理等需要绕过 Device 框架的场景）
  auto GetDriver() -> Ns16550a& { return driver_; }

 protected:
  auto DoOpen(OpenFlags flags) -> Expected<void> {
    if (!flags.CanRead() && !flags.CanWrite()) {
      return std::unexpected(Error{ErrorCode::kInvalidArgument});
    }
    flags_ = flags;
    return {};
  }

  auto DoCharRead(std::span<uint8_t> buffer) -> Expected<size_t> {
    if (!flags_.CanRead()) {
      return std::unexpected(Error{ErrorCode::kDevicePermissionDenied});
    }
    for (size_t i = 0; i < buffer.size(); ++i) {
      auto ch = driver_.TryGetChar();
      if (!ch) {
        return i;
      }
      buffer[i] = *ch;
    }
    return buffer.size();
  }

  auto DoCharWrite(std::span<const uint8_t> data) -> Expected<size_t> {
    if (!flags_.CanWrite()) {
      return std::unexpected(Error{ErrorCode::kDevicePermissionDenied});
    }
    for (auto byte : data) {
      driver_.PutChar(byte);
    }
    return data.size();
  }

  auto DoPoll(PollEvents requested) -> Expected<PollEvents> {
    uint32_t ready = 0;
    if (requested.HasIn() && driver_.HasData()) {
      ready |= PollEvents::kIn;
    }
    if (requested.HasOut()) {
      ready |= PollEvents::kOut;
    }
    return PollEvents{ready};
  }

  auto DoRelease() -> Expected<void> { return {}; }

 private:
  // CRTP 基类需要访问 DoXxx 方法
  template <class>
  friend class DeviceOperationsBase;
  template <class>
  friend class CharDevice;

  Ns16550a driver_;
  OpenFlags flags_{0};
};

#endif /* SIMPLEKERNEL_SRC_DRIVER_NS16550A_INCLUDE_NS16550A_HPP_ */
