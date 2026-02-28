/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Tick event observer interface (etl::observer pattern)
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_TICK_OBSERVER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_TICK_OBSERVER_HPP_

#include <etl/observer.h>

#include <cstdint>

#include "kernel_config.hpp"

/// @brief Tick event payload
struct TickEvent {
  uint64_t jiffies;
};

/// @brief Observer interface for tick events
using ITickObserver = etl::observer<TickEvent>;

/// @brief Observable base for tick event publishers
using TickObservable =
    etl::observable<ITickObserver, kernel::config::kTickObservers>;

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TICK_OBSERVER_HPP_
