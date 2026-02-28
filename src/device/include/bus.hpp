/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BUS_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BUS_HPP_

#include <concepts>
#include <cstddef>

#include "device_node.hpp"
#include "expected.hpp"

/// 总线 concept — 每种总线负责枚举自己管辖范围内的设备
template <typename B>
concept Bus = requires(B b, DeviceNode* out, size_t max) {
  /// 枚举该总线上所有设备，填充到 out[]，返回发现的设备数
  { b.Enumerate(out, max) } -> std::same_as<Expected<size_t>>;
  /// 返回总线名称（用于日志）
  { B::GetName() } -> std::same_as<const char*>;
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BUS_HPP_
