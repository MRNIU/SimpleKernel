/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_

#include <device_framework/ns16550a.hpp>

#include "device_node.hpp"
#include "df_bridge.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "singleton.hpp"

class Ns16550aDriver {
 public:
  using Ns16550aType = device_framework::ns16550a::Ns16550a;

  static constexpr PlatformCompatible kMatchNs16550a{"ns16550a"};
  static constexpr PlatformCompatible kMatchNs16550{"ns16550"};
  static constexpr MatchEntry kMatchTable[] = {
      kMatchNs16550a,
      kMatchNs16550,
  };

  static auto GetDescriptor() -> const DriverDescriptor& {
    static const DriverDescriptor desc{
        .name = "ns16550a",
        .match_table = kMatchTable,
        .match_count = sizeof(kMatchTable) / sizeof(kMatchTable[0]),
    };
    return desc;
  }

  auto Probe(DeviceNode& node) -> Expected<void> {
    auto result = df_bridge::ProbeWithMmio(
        node, 0x100, [](uint64_t base) { return Ns16550aType::Create(base); });
    if (!result) {
      return std::unexpected(result.error());
    }
    uart_ = std::move(*result);

    node.type = DeviceType::kChar;
    klog::Info("Ns16550aDriver: UART at 0x%lX bound\n",
               node.resource.mmio[0].base);
    return {};
  }

  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    return {};
  }

  [[nodiscard]] auto GetDevice() -> Ns16550aType* { return &uart_; }

 private:
  Ns16550aType uart_;
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_ */
