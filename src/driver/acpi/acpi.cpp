/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "acpi.h"

#include "io.hpp"

Acpi::Acpi(uint64_t rsdp) : rsdp_addr_(rsdp) {}
