/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_BUS_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_BUS_HPP_

#include "bus.hpp"
#include "device_node.hpp"
#include "expected.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "sk_cstring"

/// Platform 总线 — FDT 驱动的设备发现
class PlatformBus {
 public:
  explicit PlatformBus(KernelFdt& fdt) : fdt_(fdt) {}

  static auto GetName() -> const char* { return "platform"; }

  /// 枚举 FDT 中所有设备节点
  auto Enumerate(DeviceNode* out, size_t max) -> Expected<size_t> {
    size_t count = 0;

    auto result = fdt_.ForEachNode(
        [&out, &count, max](const char* node_name, const char* compatible,
                            uint64_t mmio_base, size_t mmio_size,
                            uint32_t irq) -> bool {
          if (count >= max) {
            return false;
          }

          // 跳过没有 compatible 的节点
          if (compatible == nullptr || compatible[0] == '\0') {
            return true;
          }

          auto& node = out[count];

          // 设备名称
          strncpy(node.name, node_name, sizeof(node.name) - 1);
          node.name[sizeof(node.name) - 1] = '\0';

          // 设备类型（默认 Platform）
          node.type = DeviceType::kPlatform;

          // MMIO
          if (mmio_base != 0) {
            node.resource.mmio[0] = {mmio_base, mmio_size};
            node.resource.mmio_count = 1;
          }

          // 中断
          if (irq != 0) {
            node.resource.irq[0] = irq;
            node.resource.irq_count = 1;
          }

          // Platform 标识
          PlatformId plat{};
          strncpy(plat.compatible, compatible, sizeof(plat.compatible) - 1);
          plat.compatible[sizeof(plat.compatible) - 1] = '\0';
          node.resource.id = plat;

          klog::Debug(
              "PlatformBus: found '%s' compatible='%s' "
              "mmio=0x%lX size=0x%lX irq=%u\n",
              node_name, compatible, mmio_base, mmio_size, irq);

          ++count;
          return true;
        });

    if (!result.has_value()) {
      return std::unexpected(result.error());
    }

    return count;
  }

  /// @name 构造/析构函数
  /// @{
  PlatformBus() = delete;
  ~PlatformBus() = default;
  PlatformBus(const PlatformBus&) = delete;
  PlatformBus(PlatformBus&&) = delete;
  auto operator=(const PlatformBus&) -> PlatformBus& = delete;
  auto operator=(PlatformBus&&) -> PlatformBus& = delete;
  /// @}

 private:
  KernelFdt& fdt_;
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_BUS_HPP_ */
