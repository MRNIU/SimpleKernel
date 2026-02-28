/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 平台特性 — 满足 device_framework VirtioTraits 约束
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_TRAITS_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_TRAITS_HPP_

#include <cpu_io.h>

#include <cstdint>

/// 平台特性（满足 EnvironmentTraits + BarrierTraits + DmaTraits）
struct PlatformTraits {
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

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_TRAITS_HPP_
