
/**
 * @file io.hpp
 * @brief 直接内存读写
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2022-01-01
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2022-01-01<td>MRNIU<td>迁移到 doxygen
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_INCLUDE_IO_HPP_
#define SIMPLEKERNEL_SRC_KERNEL_INCLUDE_IO_HPP_

#include <cpu_io.h>

#include <atomic>
#include <concepts>
#include <cstddef>

#include "per_cpu.hpp"
#include "sk_cstdio"

namespace io {
/**
 * @brief  从指定地址读数据
 * @tparam T              要读的数据类型
 * @param  addr           要读的指定地址
 * @return T        读取到的数据
 */
template <std::integral T>
static __always_inline auto In(const uint64_t addr) -> T {
  return *reinterpret_cast<volatile T *>(addr);
}

/**
 * @brief  向指定地址写数据
 * @tparam T              要写的数据类型
 * @param  addr           要写的指定地址
 * @param  data           要写的数据
 */
template <std::integral T>
static __always_inline void Out(const uint64_t addr, const T data) {
  *reinterpret_cast<volatile T *>(addr) = data;
}

}  // namespace io

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_IO_HPP_ */
