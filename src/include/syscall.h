
/**
 * @file syscall.h
 * @brief 系统调用头文件
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2022-01-30
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2022-01-30<td>MRNIU<td>创建文件
 * </table>
 */

#ifndef _SYSCALL_H_
#define _SYSCALL_H_

#include "stdint.h"

bool syscall_init(void);

static constexpr const uint8_t SYS_putc = 0;

#endif /* _SYSCALL_H_ */
