
/**
 * @file 8259a.h
 * @brief 8259a 头文件
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2024-05-24
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2024-05-24<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#include "8259a.h"

#include "io.hpp"

PIC8259A::PIC8259A(uint64_t dev_addr) : base_addr_(dev_addr) {}
