
/**
 * @file acpi.cpp
 * @brief acpi 实现
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

#include "acpi.h"

#include "io.hpp"

Acpi::Acpi(uint64_t rsdp) : rsdp_addr_(rsdp) {}
