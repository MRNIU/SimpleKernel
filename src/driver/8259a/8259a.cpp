/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 8259a 头文件
 */

#include "8259a.h"

#include "io.hpp"

PIC8259A::PIC8259A(uint64_t dev_addr) : base_addr_(dev_addr) {}
