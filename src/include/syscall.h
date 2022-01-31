
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
#include "stddef.h"

/**
 * @brief 系统调用初始化
 * @return int32_t         成功返回 0
 */
int32_t syscall_init(void);

inline int syscall(uint8_t _sysno, ...);

int32_t sys_putc(int _c);

static constexpr const size_t  MAX_ARGS = 7;
static constexpr const uint8_t SYS_putc = 0;

#endif /* _SYSCALL_H_ */
