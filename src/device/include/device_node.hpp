/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_NODE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_NODE_HPP_

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <variant>

/// Platform 设备标识（FDT compatible stringlist）
struct PlatformId {
  /// 完整的 compatible stringlist（多个字符串以 '\0' 分隔）
  char compatible[128];
  /// stringlist 的实际字节长度（包含所有 '\0' 分隔符）
  size_t compatible_len{0};
};

/// PCI 设备标识
struct PciAddress {
  /// PCI segment group
  uint16_t segment;
  uint8_t bus;
  uint8_t device;
  uint8_t function;
  uint16_t vendor_id;
  uint16_t device_id;
  uint8_t class_code;
  uint8_t subclass;
};

/// ACPI 设备标识
struct AcpiId {
  /// Hardware ID, e.g. "PNP0501"
  char hid[16];
  /// Unique ID
  char uid[16];
};

/// 总线特定标识（类型安全 variant）
using BusId = std::variant<PlatformId, PciAddress, AcpiId>;

/// 设备类型
enum class DeviceType : uint8_t {
  /// 字符设备
  kChar,
  /// 块设备
  kBlock,
  /// 网络设备
  kNet,
  /// 平台设备（中断控制器、定时器等）
  kPlatform,
};

/// 设备硬件资源描述
struct DeviceResource {
  /// MMIO 区域
  struct MmioRegion {
    uint64_t base;
    size_t size;
  };

  /// PCI 最多 6 个 BAR
  static constexpr size_t kMaxMmioRegions = 6;
  MmioRegion mmio[kMaxMmioRegions]{};
  uint8_t mmio_count{0};

  /// 中断资源
  static constexpr size_t kMaxIrqs = 4;
  uint32_t irq[kMaxIrqs]{};
  uint8_t irq_count{0};

  /// 总线特定标识
  BusId id;

  /// 便捷方法
  [[nodiscard]] auto IsPlatform() const {
    return std::holds_alternative<PlatformId>(id);
  }
  [[nodiscard]] auto IsPci() const {
    return std::holds_alternative<PciAddress>(id);
  }
  [[nodiscard]] auto IsAcpi() const {
    return std::holds_alternative<AcpiId>(id);
  }
};

/// 设备节点 — 系统中每个设备的统一表示
struct DeviceNode {
  /// 设备名称
  char name[32]{};
  /// 设备类型
  DeviceType type{};
  /// 硬件资源
  DeviceResource resource{};
  /// 是否已绑定驱动
  std::atomic<bool> bound{false};
  /// 全局设备编号
  uint32_t dev_id{0};

  /// 尝试绑定（CAS 保证幂等，防止重复绑定）
  auto TryBind() -> bool {
    bool expected = false;
    return bound.compare_exchange_strong(expected, true,
                                         std::memory_order_acq_rel);
  }

  /// @name 构造/析构函数
  /// @{
  DeviceNode() = default;
  ~DeviceNode() = default;
  DeviceNode(const DeviceNode& other)
      : type(other.type),
        resource(other.resource),
        bound(other.bound.load(std::memory_order_relaxed)),
        dev_id(other.dev_id) {
    sk_std::memcpy(name, other.name, sizeof(name));
  }
  auto operator=(const DeviceNode& other) -> DeviceNode& {
    if (this != &other) {
      sk_std::memcpy(name, other.name, sizeof(name));
      type = other.type;
      resource = other.resource;
      bound.store(other.bound.load(std::memory_order_relaxed),
                  std::memory_order_relaxed);
      dev_id = other.dev_id;
    }
    return *this;
  }
  DeviceNode(DeviceNode&&) = delete;
  auto operator=(DeviceNode&&) -> DeviceNode& = delete;
  /// @}
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_NODE_HPP_ */
