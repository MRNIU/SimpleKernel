/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief device_framework ↔ SimpleKernel 桥接层
 *
 * 提供通用 Probe 辅助模板，消除每个驱动的重复样板。
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DF_BRIDGE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DF_BRIDGE_HPP_

#include <type_traits>

#include "device_node.hpp"
#include "expected.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "virtual_memory.hpp"

namespace df_bridge {

/**
 * @brief  MMIO Probe 上下文 — TryBind + 提取 MMIO + 映射的公共流程
 */
struct MmioProbeContext {
  uint64_t base;
  size_t size;
};

/**
 * @brief  执行通用 MMIO Probe 前置流程
 * @param  node           设备节点
 * @param  default_size   默认 MMIO 映射大小（当 FDT 未给出 size 时使用）
 * @return Expected<MmioProbeContext> 成功时返回
 * MmioProbeContext；失败时节点已回滚
 */
inline auto PrepareMmioProbe(DeviceNode& node, size_t default_size)
    -> Expected<MmioProbeContext> {
  if (!node.TryBind()) {
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }

  uint64_t base = node.resource.mmio[0].base;
  if (base == 0) {
    klog::Err("df_bridge: no MMIO base for '%s'\n", node.name);
    node.Release();
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }

  size_t size = node.resource.mmio[0].size > 0 ? node.resource.mmio[0].size
                                               : default_size;

  // 映射 MMIO 区域到虚拟地址空间
  VirtualMemorySingleton::instance().MapMMIO(base, size);

  return MmioProbeContext{base, size};
}

/**
 * @brief  通用 MMIO Probe 辅助 — TryBind → 提取 MMIO → 映射 → 调用 create_fn
 *
 * @tparam CreateFn       可调用类型: (uint64_t base) ->
 * device_framework::Expected<T>
 * @param  node           设备节点
 * @param  default_size   默认 MMIO 映射大小
 * @param  create_fn      设备工厂函数
 * @return Expected<std::remove_reference_t<decltype(*create_fn(uint64_t{}))>>
 * 成功时返回设备实例；失败时节点已回滚
 */
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
    node.Release();
    return std::unexpected(Error(result.error().code));
  }

  return std::move(*result);
}

}  // namespace df_bridge

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DF_BRIDGE_HPP_
