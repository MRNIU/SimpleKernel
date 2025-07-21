/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief apic 头文件
 */

#include "apic.h"

#include "io.hpp"

Apic::Apic(uint64_t dev_addr) : base_addr_(dev_addr) {}
