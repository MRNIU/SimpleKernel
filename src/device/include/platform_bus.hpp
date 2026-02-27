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
#include "kstd_cstring"

/// Platform 总线 — FDT 驱动的设备发现
class PlatformBus {
 public:
  explicit PlatformBus(KernelFdt& fdt) : fdt_(fdt) {}

  static auto GetName() -> const char* { return "platform"; }

  /// 枚举 FDT 中所有设备节点
  auto Enumerate(DeviceNode* out, size_t max) -> Expected<size_t> {
    size_t count = 0;

    auto result = fdt_.ForEachNode(
        [&out, &count, max](const char* node_name, const char* compatible_data,
                            size_t compatible_len, uint64_t mmio_base,
                            size_t mmio_size, uint32_t irq) -> bool {
          if (count >= max) {
            return false;
          }

          if (compatible_data == nullptr || compatible_len == 0) {
            return true;
          }

          auto& node = out[count];

          strncpy(node.name, node_name, sizeof(node.name) - 1);
          node.name[sizeof(node.name) - 1] = '\0';

          node.type = DeviceType::kPlatform;

          if (mmio_base != 0) {
            node.resource.mmio[0] = {mmio_base, mmio_size};
            node.resource.mmio_count = 1;
          }

          if (irq != 0) {
            node.resource.irq[0] = irq;
            node.resource.irq_count = 1;
          }

          PlatformId plat{};
          if (compatible_len > sizeof(plat.compatible)) {
            klog::Warn(
                "PlatformBus: compatible data truncated from %zu to %zu bytes "
                "for node '%s'\\n",
                compatible_len, sizeof(plat.compatible), node_name);
          }
          size_t copy_len = compatible_len < sizeof(plat.compatible)
                                ? compatible_len
                                : sizeof(plat.compatible);
          memcpy(plat.compatible, compatible_data, copy_len);
          plat.compatible_len = copy_len;
          node.resource.id = plat;

          klog::Debug(
              "PlatformBus: found '%s' compatible='%s' "
              "mmio=0x%lX size=0x%lX irq=%u\n",
              node_name, compatible_data, mmio_base, mmio_size, irq);

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
