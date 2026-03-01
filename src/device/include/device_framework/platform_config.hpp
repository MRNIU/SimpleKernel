/**
 * @copyright Copyright The SimpleKernel Contributors
 */

/// @file platform_config.hpp
/// @brief Concrete implementation of device_framework platform_config.hpp for
///        SimpleKernel.
///
/// This file is placed at
/// src/device/include/device_framework/platform_config.hpp and takes priority
/// over 3rd/device_framework/include/device_framework/ platform_config.hpp due
/// to include path ordering in src/device/CMakeLists.txt (BEFORE keyword
/// ensures our include directory is prepended).

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_

#include <cpu_io.h>

#include <cstdint>

#include "device_framework/traits.hpp"
#include "expected.hpp"

/// Platform traits satisfying EnvironmentTraits, BarrierTraits, DmaTraits,
/// and SpinWaitTraits (inlined from the former platform_traits.hpp).
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

// PlatformTraits satisfies EnvironmentTraits, BarrierTraits, and DmaTraits.
using PlatformEnvironment = PlatformTraits;
using PlatformBarrier = PlatformTraits;
using PlatformDma = PlatformTraits;

// Use the kernel's ErrorCode directly.  device_framework's ErrorCode becomes
// an alias for this type, eliminating the ToKernelErrorCode() conversion layer.
using PlatformErrorCode = ::ErrorCode;  // kernel root-namespace ErrorCode

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_ \
        */
