/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief PL011 UART driver public interface
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_PL011_PL011_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_PL011_PL011_DRIVER_HPP_

#include "pl011/pl011.hpp"

namespace pl011 {
/// Type alias for singleton usage (etl::singleton<pl011::Pl011Device>)
using Pl011Device = Pl011;
}  // namespace pl011

#endif  // SIMPLEKERNEL_SRC_DEVICE_PL011_PL011_DRIVER_HPP_
