/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief device_framework ↔ SimpleKernel 桥接层
 *
 * 提供错误类型映射和通用 Probe 辅助模板，消除每个驱动的重复样板。
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DF_BRIDGE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DF_BRIDGE_HPP_

#include <device_framework/expected.hpp>
#include <type_traits>

#include "device_node.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "virtual_memory.hpp"

namespace df_bridge {

/// @brief 类型安全的设备存储 — 在固定内存中管理设备生命周期
///
/// 使用 placement new/delete 在内联缓冲区中构造和销毁设备实例，
/// 替代手动 StorageBlock + reinterpret_cast 模式。
///
/// @tparam T 设备类型
template <typename T>
class DeviceStorage {
 public:
  /// @brief 在存储中原地构造设备实例（若已有实例则先销毁）
  template <typename... Args>
  auto Emplace(Args&&... args) -> T& {
    Destroy();
    new (data_) T(static_cast<Args&&>(args)...);
    valid_ = true;
    return *reinterpret_cast<T*>(data_);
  }

  /// @brief 获取设备指针（无效时返回 nullptr）
  [[nodiscard]] auto Get() -> T* {
    return valid_ ? reinterpret_cast<T*>(data_) : nullptr;
  }

  /// @brief 获取设备指针（const 版本）
  [[nodiscard]] auto Get() const -> const T* {
    return valid_ ? reinterpret_cast<const T*>(data_) : nullptr;
  }

  /// @brief 是否持有有效实例
  [[nodiscard]] auto IsValid() const -> bool { return valid_; }

  /// @brief 销毁设备实例
  auto Destroy() -> void {
    if (valid_) {
      reinterpret_cast<T*>(data_)->~T();
      valid_ = false;
    }
  }

  ~DeviceStorage() { Destroy(); }
  DeviceStorage() = default;
  DeviceStorage(const DeviceStorage&) = delete;
  auto operator=(const DeviceStorage&) -> DeviceStorage& = delete;
  DeviceStorage(DeviceStorage&&) = delete;
  auto operator=(DeviceStorage&&) -> DeviceStorage& = delete;

 private:
  alignas(alignof(T)) uint8_t data_[sizeof(T)]{};
  bool valid_{false};
};

/// @brief 将 device_framework::ErrorCode 映射为 SimpleKernel ErrorCode
/// @param code device_framework 错误码
/// @return 内核错误码
constexpr auto ToKernelErrorCode(device_framework::ErrorCode code)
    -> ErrorCode {
  switch (code) {
    case device_framework::ErrorCode::kSuccess:
      return ErrorCode::kSuccess;
    case device_framework::ErrorCode::kInvalidArgument:
      return ErrorCode::kInvalidArgument;
    case device_framework::ErrorCode::kOutOfMemory:
      return ErrorCode::kOutOfMemory;

    // 设备操作层错误 → 内核 Device 错误
    case device_framework::ErrorCode::kDeviceAlreadyOpen:
      return ErrorCode::kDeviceAlreadyOpen;
    case device_framework::ErrorCode::kDeviceNotOpen:
      return ErrorCode::kDeviceNotOpen;
    case device_framework::ErrorCode::kDeviceNotSupported:
      return ErrorCode::kDeviceNotSupported;
    case device_framework::ErrorCode::kDevicePermissionDenied:
      return ErrorCode::kDevicePermissionDenied;
    case device_framework::ErrorCode::kDeviceBlockUnaligned:
      return ErrorCode::kDeviceBlockUnaligned;
    case device_framework::ErrorCode::kDeviceBlockOutOfRange:
      return ErrorCode::kDeviceBlockOutOfRange;
    case device_framework::ErrorCode::kDeviceReadFailed:
      return ErrorCode::kDeviceReadFailed;

    // 通用设备错误
    case device_framework::ErrorCode::kDeviceError:
    case device_framework::ErrorCode::kIoError:
      return ErrorCode::kDeviceReadFailed;
    case device_framework::ErrorCode::kNotSupported:
      return ErrorCode::kDeviceNotSupported;
    case device_framework::ErrorCode::kTimeout:
      return ErrorCode::kDeviceBusy;

    // 传输层 / 队列错误 → 视为设备不可用
    default:
      return ErrorCode::kDeviceNotFound;
  }
}

/// @brief 将 device_framework::Error 映射为 SimpleKernel Error
/// @param err device_framework 错误
/// @return 内核 Error
constexpr auto ToKernelError(const device_framework::Error& err) -> Error {
  return Error(ToKernelErrorCode(err.code));
}

/// @brief MMIO Probe 上下文 — TryBind + 提取 MMIO + 映射的公共流程
struct MmioProbeContext {
  uint64_t base;
  size_t size;
};

/// @brief 执行通用 MMIO Probe 前置流程
/// @param node 设备节点
/// @param default_size 默认 MMIO 映射大小（当 FDT 未给出 size 时使用）
/// @return 成功时返回 MmioProbeContext；失败时节点已回滚
inline auto PrepareMmioProbe(DeviceNode& node, size_t default_size)
    -> Expected<MmioProbeContext> {
  if (!node.TryBind()) {
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }

  uint64_t base = node.resource.mmio[0].base;
  if (base == 0) {
    klog::Err("df_bridge: no MMIO base for '%s'\n", node.name);
    node.bound.store(false, std::memory_order_release);
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }

  size_t size = node.resource.mmio[0].size > 0 ? node.resource.mmio[0].size
                                               : default_size;

  // 映射 MMIO 区域到虚拟地址空间
  Singleton<VirtualMemory>::GetInstance().MapMMIO(base, size);

  return MmioProbeContext{base, size};
}

/// @brief 通用 MMIO Probe 辅助 — TryBind → 提取 MMIO → 映射 → 调用 create_fn
///
/// @tparam CreateFn 可调用类型: (uint64_t base) ->
/// device_framework::Expected<T>
/// @param node 设备节点
/// @param default_size 默认 MMIO 映射大小
/// @param create_fn 设备工厂函数
/// @return 成功时返回设备实例；失败时节点已回滚
template <typename CreateFn>
auto ProbeWithMmio(DeviceNode& node, size_t default_size, CreateFn&& create_fn)
    -> Expected<std::remove_reference_t<decltype(*create_fn(uint64_t{}))>> {
  auto ctx = PrepareMmioProbe(node, default_size);
  if (!ctx) {
    return std::unexpected(ctx.error());
  }

  auto result = create_fn(ctx->base);
  if (!result) {
    klog::Err("df_bridge: device creation failed at 0x%lX: %s\n", ctx->base,
              result.error().message());
    node.bound.store(false, std::memory_order_release);
    return std::unexpected(ToKernelError(result.error()));
  }

  return std::move(*result);
}

}  // namespace df_bridge

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DF_BRIDGE_HPP_ */
