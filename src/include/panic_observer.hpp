/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#pragma once

#include <etl/observer.h>

#include <cstdint>

#include "kernel_config.hpp"

/// Panic event payload
struct PanicEvent {
  const char* reason;
  uint64_t pc;
};

/// Observer interface for panic events
using IPanicObserver = etl::observer<PanicEvent>;

/// Observable base for panic event publishers
using PanicObservable =
    etl::observable<IPanicObserver, kernel::config::kPanicObservers>;
