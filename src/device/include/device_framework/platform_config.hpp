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

#include "device_framework/traits.hpp"
#include "expected.hpp"
#include "platform_traits.hpp"

// PlatformTraits satisfies EnvironmentTraits, BarrierTraits, and DmaTraits.
// See src/device/include/platform_traits.hpp for the implementation.
using PlatformEnvironment = PlatformTraits;
using PlatformBarrier = PlatformTraits;
using PlatformDma = PlatformTraits;

// Use the kernel's ErrorCode directly.  device_framework's ErrorCode becomes
// an alias for this type, eliminating the ToKernelErrorCode() conversion layer.
using PlatformErrorCode = ::ErrorCode;  // kernel root-namespace ErrorCode

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_ \
        */
