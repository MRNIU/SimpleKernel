/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief NS16550A UART driver
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_

#include <device_framework/ns16550a.hpp>

#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "mmio_helper.hpp"

class Ns16550aDriver {
 public:
  using Ns16550aType = device_framework::ns16550a::Ns16550a;

  // --- Registration API ---

  /// Return the singleton driver instance.
  static auto Instance() -> Ns16550aDriver& {
    static Ns16550aDriver inst;
    return inst;
  }

  /// Return the DriverEntry for registration.
  static auto GetEntry() -> const DriverEntry& {
    static const DriverEntry entry{
        .descriptor = &kDescriptor,
        .match = MatchStatic,
        .probe = [](DeviceNode& n) { return Instance().Probe(n); },
        .remove = [](DeviceNode& n) { return Instance().Remove(n); },
    };
    return entry;
  }

  // --- Driver lifecycle ---

  /**
   * @brief Hardware detection: does the MMIO region respond as NS16550A?
   *
   * For NS16550A this is a compatible-string-only match — no MMIO reads
   * needed (the device has no readable signature register).
   *
   * @return true if node.compatible contains "ns16550a" or "ns16550"
   */
  static auto MatchStatic([[maybe_unused]] DeviceNode& node) -> bool {
    // Compatible match was already done by FindDriver(); return true to
    // confirm.
    return true;
  }

  /**
   * @brief Initialize the NS16550A UART.
   *
   * @pre  node.mmio_base != 0
   * @post uart_ is valid; node.type == DeviceType::kChar
   */
  auto Probe(DeviceNode& node) -> Expected<void> {
    auto ctx = mmio_helper::Prepare(node, 0x100);
    if (!ctx) {
      return std::unexpected(ctx.error());
    }

    auto result = Ns16550aType::Create(ctx->base);
    if (!result) {
      return std::unexpected(Error(result.error().code));
    }

    uart_ = std::move(*result);
    node.type = DeviceType::kChar;
    klog::Info("Ns16550aDriver: UART at 0x%lX bound\n", node.mmio_base);
    return {};
  }

  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    return {};
  }

  [[nodiscard]] auto GetDevice() -> Ns16550aType* { return &uart_; }

 private:
  static constexpr MatchEntry kMatchTable[] = {
      {BusType::kPlatform, "ns16550a"},
      {BusType::kPlatform, "ns16550"},
  };
  static const DriverDescriptor kDescriptor;

  Ns16550aType uart_;
};

inline const DriverDescriptor Ns16550aDriver::kDescriptor{
    .name = "ns16550a",
    .match_table = kMatchTable,
    .match_count = sizeof(kMatchTable) / sizeof(kMatchTable[0]),
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_
