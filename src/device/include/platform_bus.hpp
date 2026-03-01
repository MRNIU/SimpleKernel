/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Platform bus — FDT-driven device discovery
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_BUS_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_BUS_HPP_

#include "bus.hpp"
#include "device_node.hpp"
#include "expected.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "kstd_cstring"

/// Platform bus — enumerates devices from the Flattened Device Tree (FDT)
class PlatformBus {
 public:
  explicit PlatformBus(KernelFdt& fdt) : fdt_(fdt) {}

  static auto GetName() -> const char* { return "platform"; }

  /**
   * @brief Enumerate all FDT device nodes with a compatible string.
   *
   * @param  out  Output array of DeviceNode
   * @param  max  Maximum number of nodes to write
   * @return Expected<size_t> number of nodes written
   */
  auto Enumerate(DeviceNode* out, size_t max) -> Expected<size_t> {
    size_t count = 0;

    auto result = fdt_.ForEachNode(
        [&out, &count, max](const char* node_name, const char* compatible_data,
                            size_t compatible_len, uint64_t mmio_base,
                            size_t mmio_size, uint32_t irq) -> bool {
          if (count >= max) return false;
          if (compatible_data == nullptr || compatible_len == 0) return true;

          auto& node = out[count];

          kstd::strncpy(node.name, node_name, sizeof(node.name) - 1);
          node.name[sizeof(node.name) - 1] = '\0';

          node.bus_type = BusType::kPlatform;
          node.type = DeviceType::kPlatform;
          node.mmio_base = mmio_base;
          node.mmio_size = mmio_size;
          node.irq = irq;

          size_t copy_len = compatible_len < sizeof(node.compatible)
                                ? compatible_len
                                : sizeof(node.compatible);
          if (compatible_len > sizeof(node.compatible)) {
            klog::Warn(
                "PlatformBus: compatible truncated %zu\u2192%zu for '%s'\n",
                compatible_len, sizeof(node.compatible), node_name);
          }
          kstd::memcpy(node.compatible, compatible_data, copy_len);
          node.compatible_len = copy_len;

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

  PlatformBus() = delete;
  ~PlatformBus() = default;
  PlatformBus(const PlatformBus&) = delete;
  PlatformBus(PlatformBus&&) = delete;
  auto operator=(const PlatformBus&) -> PlatformBus& = delete;
  auto operator=(PlatformBus&&) -> PlatformBus& = delete;

 private:
  KernelFdt& fdt_;
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_BUS_HPP_
