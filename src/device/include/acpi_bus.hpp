/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief ACPI 总线 — 通过 ACPI 表枚举设备
 * @todo 实现 ACPI DSDT/SSDT 设备枚举
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_ACPI_BUS_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_ACPI_BUS_HPP_

#include "bus.hpp"
#include "device_node.hpp"
#include "expected.hpp"

/// @todo 实现 ACPI 总线枚举
class AcpiBus {
 public:
  /// @param rsdp RSDP 地址（由 firmware/bootloader 传入）
  explicit AcpiBus(uint64_t rsdp) : rsdp_(rsdp) {}

  static auto GetName() -> const char* { return "acpi"; }

  /// @todo 实现 ACPI 设备枚举
  auto Enumerate([[maybe_unused]] DeviceNode* out, [[maybe_unused]] size_t max)
      -> Expected<size_t> {
    // ACPI 表解析尚未实现
    return static_cast<size_t>(0);
  }

  /// @name 构造/析构函数
  /// @{
  AcpiBus() = delete;
  ~AcpiBus() = default;
  AcpiBus(const AcpiBus&) = delete;
  AcpiBus(AcpiBus&&) = delete;
  auto operator=(const AcpiBus&) -> AcpiBus& = delete;
  auto operator=(AcpiBus&&) -> AcpiBus& = delete;
  /// @}

 private:
  uint64_t rsdp_;
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_ACPI_BUS_HPP_ */
