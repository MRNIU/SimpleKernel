/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_NS16550A_NS16550A_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_NS16550A_NS16550A_DRIVER_HPP_

#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "ns16550a/ns16550a.hpp"

class Ns16550aDriver {
 public:
  using Ns16550aType = ns16550a::Ns16550a;

  /// 返回驱动单例实例。
  static auto Instance() -> Ns16550aDriver& {
    static Ns16550aDriver inst;
    return inst;
  }

  /// 返回用于注册的 DriverEntry。
  static auto GetEntry() -> const DriverEntry& {
    static const DriverEntry entry{
        .name = "ns16550a",
        .match_table = etl::span<const MatchEntry>(kMatchTable),
        .match = etl::delegate<bool(
            DeviceNode&)>::create<&Ns16550aDriver::MatchStatic>(),
        .probe = etl::delegate<Expected<void>(DeviceNode&)>::create<
            Ns16550aDriver, &Ns16550aDriver::Probe>(Instance()),
        .remove = etl::delegate<Expected<void>(DeviceNode&)>::create<
            Ns16550aDriver, &Ns16550aDriver::Remove>(Instance()),
    };
    return entry;
  }

  /**
   * @brief 硬件检测：MMIO 区域大小是否足够 NS16550A？
   *
   * NS16550A 没有可读的 magic 寄存器，仅验证 MMIO 资源。
   *
   * @return true 如果 mmio_base 非零且 mmio_size 足够
   */
  static auto MatchStatic([[maybe_unused]] DeviceNode& node) -> bool {
    static constexpr size_t kMinRegisterSpace = 8;
    return node.mmio_base != 0 && node.mmio_size >= kMinRegisterSpace;
  }

  /**
   * @brief 初始化 NS16550A UART。
   *
   * @pre  node.mmio_base != 0
   * @post uart_ 有效；node.type == DeviceType::kChar
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
    klog::info << "Ns16550aDriver: UART at " << klog::hex << node.mmio_base
               << " bound";
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

  Ns16550aType uart_;
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_NS16550A_NS16550A_DRIVER_HPP_
