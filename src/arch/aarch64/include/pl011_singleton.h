/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief AArch64 PL011 singleton type alias
 */

#ifndef SIMPLEKERNEL_SRC_ARCH_AARCH64_INCLUDE_PL011_SINGLETON_H_
#define SIMPLEKERNEL_SRC_ARCH_AARCH64_INCLUDE_PL011_SINGLETON_H_

#include <etl/singleton.h>

#include <device_framework/pl011.hpp>

using Pl011Singleton = etl::singleton<device_framework::pl011::Pl011Device>;

#endif /* SIMPLEKERNEL_SRC_ARCH_AARCH64_INCLUDE_PL011_SINGLETON_H_ */
