/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_

#include <device_framework/ns16550a.hpp>

#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "singleton.hpp"
#include "virtual_memory.hpp"

/// NS16550A UART 驱动
class Ns16550aDriver {
 public:
  using Ns16550aType = device_framework::ns16550a::Ns16550a;

  /// 驱动匹配表 — FDT compatible 字符串
  static constexpr PlatformCompatible kMatchNs16550a{"ns16550a"};
  static constexpr PlatformCompatible kMatchNs16550{"ns16550"};
  static constexpr MatchEntry kMatchTable[] = {
      kMatchNs16550a,
      kMatchNs16550,
  };

  /// 驱动描述符
  static auto GetDescriptor() -> const DriverDescriptor& {
    static const DriverDescriptor desc{
        .name = "ns16550a",
        .match_table = kMatchTable,
        .match_count = sizeof(kMatchTable) / sizeof(kMatchTable[0]),
    };
    return desc;
  }

  /// Probe — 初始化 NS16550A UART
  auto Probe(DeviceNode& node) -> Expected<void> {
    if (!node.TryBind()) {
      return std::unexpected(Error(ErrorCode::kDeviceNotFound));
    }

    uint64_t base = node.resource.mmio[0].base;
    if (base == 0) {
      klog::Err("Ns16550aDriver: no MMIO base for '%s'\n", node.name);
      node.bound.store(false, std::memory_order_release);
      return std::unexpected(Error(ErrorCode::kDeviceNotFound));
    }

    // 映射 MMIO 区域
    size_t size =
        node.resource.mmio[0].size > 0 ? node.resource.mmio[0].size : 0x100;
    Singleton<VirtualMemory>::GetInstance().MapMMIO(base, size);

    // 创建 NS16550A 实例（构造函数中完成初始化）
    uart_ = Ns16550aType(base);

    node.type = DeviceType::kChar;

    klog::Info("Ns16550aDriver: UART at 0x%lX bound\n", base);
    return {};
  }

  /// Remove
  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    return {};
  }

  /// 获取 UART 实例
  [[nodiscard]] auto GetDevice() -> Ns16550aType* { return &uart_; }

 private:
  Ns16550aType uart_;
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_ */
