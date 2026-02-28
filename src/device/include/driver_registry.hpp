/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 驱动注册表、Driver concept 与匹配机制
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_REGISTRY_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_REGISTRY_HPP_

#include <etl/flat_map.h>

#include <cstddef>
#include <cstdint>
#include <variant>

#include "device_node.hpp"
#include "expected.hpp"
#include "kstd_cstring"
#include "spinlock.hpp"

/// @name 驱动匹配键 tagged types
/// @{

/// Platform 匹配键（FDT compatible 字符串）
struct PlatformCompatible {
  const char* compatible;
};

/// PCI 匹配键（vendor + device ID）
struct PciMatchKey {
  uint16_t vendor_id;
  uint16_t device_id;
};

/// ACPI 匹配键（Hardware ID）
struct AcpiHid {
  const char* hid;
};

/// @}

/// 驱动匹配表条目
using MatchEntry = std::variant<PlatformCompatible, PciMatchKey, AcpiHid>;

/// 驱动描述符
struct DriverDescriptor {
  const char* name;
  const MatchEntry* match_table;
  size_t match_count;
};

/// 驱动 concept — 所有驱动必须满足
template <typename D>
concept Driver = requires(D d, DeviceNode& node) {
  { D::GetDescriptor() } -> std::same_as<const DriverDescriptor&>;
  { d.Probe(node) } -> std::same_as<Expected<void>>;
  { d.Remove(node) } -> std::same_as<Expected<void>>;
};

/// 类型擦除的驱动注册项
struct DriverEntry {
  const DriverDescriptor* descriptor;
  auto (*probe)(DeviceNode&) -> Expected<void>;
  auto (*remove)(DeviceNode&) -> Expected<void>;
};

/// @brief FNV-1a 32-bit hash of a null-terminated string (constexpr)
[[nodiscard]] constexpr auto Fnv1a32(const char* str) -> uint32_t {
  uint32_t hash = 2166136261u;
  for (; *str != '\0'; ++str) {
    hash ^= static_cast<uint8_t>(*str);
    hash *= 16777619u;
  }
  return hash;
}

/// 驱动注册表
class DriverRegistry {
 public:
  /// @brief 获取指定驱动类型的持久单例实例
  ///
  /// 每个驱动类型 D 有且仅有一个实例，在首次调用时构造，
  /// 生存期持续到程序结束。DriverRegistry 在 Probe/Remove 时
  /// 操作此实例，外部也可通过此方法访问驱动状态。
  ///
  /// @tparam D 驱动类型（必须满足 Driver concept）
  /// @return 驱动实例的引用
  template <Driver D>
  static auto GetDriverInstance() -> D& {
    static D instance;
    return instance;
  }

  /// 注册一个驱动（编译期类型安全，运行期类型擦除存储）
  template <Driver D>
  auto Register() -> Expected<void> {
    LockGuard guard(lock_);
    if (count_ >= kMaxDrivers) {
      return std::unexpected(Error(ErrorCode::kOutOfMemory));
    }

    (void)GetDriverInstance<D>();

    const size_t idx = count_;
    drivers_[idx] = DriverEntry{
        .descriptor = &D::GetDescriptor(),
        .probe = [](DeviceNode& node) -> Expected<void> {
          return GetDriverInstance<D>().Probe(node);
        },
        .remove = [](DeviceNode& node) -> Expected<void> {
          return GetDriverInstance<D>().Remove(node);
        },
    };
    ++count_;

    // Index every PlatformCompatible match string for O(log N) lookup
    const auto* desc = drivers_[idx].descriptor;
    for (size_t j = 0; j < desc->match_count; ++j) {
      if (std::holds_alternative<PlatformCompatible>(desc->match_table[j])) {
        const auto* compat =
            std::get<PlatformCompatible>(desc->match_table[j]).compatible;
        platform_index_.insert({Fnv1a32(compat), idx});
      }
    }

    return {};
  }

  /// 根据设备资源查找匹配的驱动
  auto FindDriver(const DeviceResource& resource) -> DriverEntry* {
    // Fast O(log N) path for platform devices
    if (resource.IsPlatform()) {
      const auto& plat = std::get<PlatformId>(resource.id);
      const char* p = plat.compatible;
      const char* end = plat.compatible + plat.compatible_len;
      while (p < end) {
        auto it = platform_index_.find(Fnv1a32(p));
        if (it != platform_index_.end()) {
          return &drivers_[it->second];
        }
        p += strlen(p) + 1;
      }
      return nullptr;
    }

    // Linear scan for PCI / ACPI (few entries in practice)
    for (size_t i = 0; i < count_; ++i) {
      const auto* desc = drivers_[i].descriptor;
      for (size_t j = 0; j < desc->match_count; ++j) {
        if (MatchesPciAcpi(desc->match_table[j], resource)) {
          return &drivers_[i];
        }
      }
    }
    return nullptr;
  }

  /// @name 构造/析构函数
  /// @{
  DriverRegistry() = default;
  ~DriverRegistry() = default;
  DriverRegistry(const DriverRegistry&) = delete;
  DriverRegistry(DriverRegistry&&) = delete;
  auto operator=(const DriverRegistry&) -> DriverRegistry& = delete;
  auto operator=(DriverRegistry&&) -> DriverRegistry& = delete;
  /// @}

 private:
  /// PCI/ACPI-only match (platform handled by fast-path above)
  static auto MatchesPciAcpi(const MatchEntry& entry,
                             const DeviceResource& resource) -> bool {
    return std::visit(
        [&resource](const auto& key) -> bool {
          using T = std::decay_t<decltype(key)>;

          if constexpr (std::is_same_v<T, PciMatchKey>) {
            if (!resource.IsPci()) return false;
            const auto& pci = std::get<PciAddress>(resource.id);
            return pci.vendor_id == key.vendor_id &&
                   pci.device_id == key.device_id;
          } else if constexpr (std::is_same_v<T, AcpiHid>) {
            if (!resource.IsAcpi()) return false;
            const auto& acpi = std::get<AcpiId>(resource.id);
            return strcmp(acpi.hid, key.hid) == 0;
          }
          return false;
        },
        entry);
  }

  static constexpr size_t kMaxDrivers = 32;
  DriverEntry drivers_[kMaxDrivers]{};
  size_t count_{0};
  // Platform compatible string → drivers_ index (O(log N) lookup)
  etl::flat_map<uint32_t, size_t, kMaxDrivers> platform_index_;
  SpinLock lock_{"driver_registry"};
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_REGISTRY_HPP_
