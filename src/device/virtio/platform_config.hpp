/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Kernel-native platform configuration for inlined VirtIO driver.
 *
 * Provides PlatformBarrier, PlatformEnvironment, and PlatformDma as
 * kernel-native types, replacing the former device_framework submodule's
 * platform_config.hpp.
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_VIRTIO_PLATFORM_CONFIG_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_VIRTIO_PLATFORM_CONFIG_HPP_

#include <cpu_io.h>

#include <cstdint>

/// Platform traits satisfying EnvironmentTraits, BarrierTraits, DmaTraits,
/// and SpinWaitTraits for use by the inlined VirtIO driver headers.
struct PlatformTraits {
  static constexpr uint32_t kMaxSpinIterations = 100000000;
  static auto Log(const char* /*fmt*/, ...) -> int { return 0; }
  static auto Mb() -> void { cpu_io::Mb(); }
  static auto Rmb() -> void { cpu_io::Rmb(); }
  static auto Wmb() -> void { cpu_io::Wmb(); }
  static auto VirtToPhys(void* p) -> uintptr_t {
    return reinterpret_cast<uintptr_t>(p);
  }
  static auto PhysToVirt(uintptr_t a) -> void* {
    return reinterpret_cast<void*>(a);
  }
};

using PlatformEnvironment = PlatformTraits;
using PlatformBarrier = PlatformTraits;
using PlatformDma = PlatformTraits;

#endif  // SIMPLEKERNEL_SRC_DEVICE_VIRTIO_PLATFORM_CONFIG_HPP_
