/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DRIVER_INCLUDE_DRIVER_H_
#define SIMPLEKERNEL_SRC_DRIVER_INCLUDE_DRIVER_H_

#include <cstdint>

/**
 * @brief 入口
 * @param argc 参数个数
 * @param argv 参数列表
 * @return uint32_t                 正常返回 0
 */
auto Driver(uint32_t argc, const uint8_t *argv) -> uint32_t;

#endif /* SIMPLEKERNEL_SRC_DRIVER_INCLUDE_DRIVER_H_ */
