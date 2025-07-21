/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief acpi 实现
 */

#include "acpi.h"

#include "io.hpp"

Acpi::Acpi(uint64_t rsdp) : rsdp_addr_(rsdp) {}
