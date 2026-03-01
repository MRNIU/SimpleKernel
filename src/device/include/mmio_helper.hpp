/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Minimal MMIO probe helper — maps a DeviceNode's MMIO region
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_MMIO_HELPER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_MMIO_HELPER_HPP_

#include "device_node.hpp"
#include "expected.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "virtual_memory.hpp"

namespace mmio_helper {

/// MMIO region after mapping
struct ProbeContext {
  uint64_t base;
  size_t size;
};

/**
 * @brief Extract MMIO base/size from node and map region via VirtualMemory.
 *
 * Does NOT set node.bound — caller (driver Probe()) is responsible for
 * setting node.bound = true under DeviceManager::lock_.
 *
 * @param  node         Device node (must have mmio_base != 0)
 * @param  default_size Fallback size when node.mmio_size == 0
 * @return Expected<ProbeContext>; kDeviceNotFound if mmio_base == 0
 */
[[nodiscard]] inline auto Prepare(const DeviceNode& node, size_t default_size)
    -> Expected<ProbeContext> {
  if (node.mmio_base == 0) {
    klog::Err("mmio_helper: no MMIO base for '%s'\n", node.name);
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }

  size_t size = node.mmio_size > 0 ? node.mmio_size : default_size;
  VirtualMemorySingleton::instance().MapMMIO(node.mmio_base, size);

  return ProbeContext{node.mmio_base, size};
}

}  // namespace mmio_helper

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_MMIO_HELPER_HPP_
