/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PCI_BUS_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PCI_BUS_HPP_

#include "bus.hpp"
#include "device_node.hpp"
#include "expected.hpp"

/**
 * @brief  PCI 总线枚举
 * @todo   实现 PCI 总线枚举
 */
class PciBus {
 public:
  /**
   * @brief  构造函数
   * @param  ecam_base      ECAM 基地址（从 FDT 或 ACPI MCFG 获取）
   */
  explicit PciBus(uint64_t ecam_base) : ecam_base_(ecam_base) {}

  /**
   * @brief  获取总线名称
   * @return const char*    总线名称
   */
  static auto GetName() -> const char* { return "pci"; }

  /**
   * @brief  设备枚举
   * @todo   实现 PCI 设备枚举
   * @param  out            输出设备节点指针数组
   * @param  max            最大枚举数量
   * @return Expected<size_t> 成功时返回实际枚举数量，失败时返回错误
   */
  auto Enumerate([[maybe_unused]] DeviceNode* out, [[maybe_unused]] size_t max)
      -> Expected<size_t> {
    // PCI ECAM 扫描尚未实现
    return static_cast<size_t>(0);
  }

  /// @name 构造/析构函数
  /// @{
  PciBus() = delete;
  ~PciBus() = default;
  PciBus(const PciBus&) = delete;
  PciBus(PciBus&&) = delete;
  auto operator=(const PciBus&) -> PciBus& = delete;
  auto operator=(PciBus&&) -> PciBus& = delete;
  /// @}

 private:
  uint64_t ecam_base_;
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PCI_BUS_HPP_ */
