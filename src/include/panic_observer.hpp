/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Panic event observer interface (etl::observer pattern)
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_PANIC_OBSERVER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_PANIC_OBSERVER_HPP_

#include <etl/observer.h>

#include <cstdint>

#include "kernel_config.hpp"

/// @brief Panic event payload
struct PanicEvent {
  const char* reason;
  uint64_t pc;
};

/// @brief Observer interface for panic events
using IPanicObserver = etl::observer<PanicEvent>;

/// @brief Observable base for panic event publishers
using PanicObservable =
    etl::observable<IPanicObserver, kernel::config::kPanicObservers>;

#endif  // SIMPLEKERNEL_SRC_INCLUDE_PANIC_OBSERVER_HPP_
