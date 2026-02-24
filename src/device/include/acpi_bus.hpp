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

/**
 * @brief  ACPI 总线 — 通过 ACPI 表枚举设备
 * @todo   实现 ACPI 总线枚举
 */
class AcpiBus {
 public:
  /**
   * @brief  构造函数
   * @param  rsdp           RSDP 地址（由 firmware/bootloader 传入）
   */
  explicit AcpiBus(uint64_t rsdp) : rsdp_(rsdp) {}

  /**
   * @brief  获取总线名称
   * @return const char*    总线名称
   */
  static auto GetName() -> const char* { return "acpi"; }

  /**
   * @brief  设备枚举
   * @todo   实现 ACPI 设备枚举
   * @param  out            输出设备节点指针数组
   * @param  max            最大枚举数量
   * @return Expected<size_t> 成功时返回实际枚举数量，失败时返回错误
   */
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
