/**
 * @file sk_ctype.h
 * @brief sk_ctype 头文件
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-07-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-07-15<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_LIBC_INCLUDE_SK_CTYPE_H_
#define SIMPLEKERNEL_SRC_KERNEL_LIBC_INCLUDE_SK_CTYPE_H_

#ifdef __cplusplus
extern "C" {
#endif

// 检查字符是否为字母或数字
int isalnum(int c);
// 检查字符是否为字母
int isalpha(int c);
// 检查字符是否为空白字符（空格或制表符）
int isblank(int c);
// 检查字符是否为控制字符
int iscntrl(int c);
// 检查字符是否为十进制数字（0-9）
int isdigit(int c);
// 检查字符是否为可打印字符（不包括空格）
int isgraph(int c);
// 检查字符是否为小写字母
int islower(int c);
// 检查字符是否为可打印字符（包括空格）
int isprint(int c);
// 检查字符是否为标点符号
int ispunct(int c);
// 检查字符是否为空白字符（空格、制表符、换行符等）
int isspace(int c);
// 检查字符是否为大写字母
int isupper(int c);
// 检查字符是否为十六进制数字（0-9、a-f、A-F）
int isxdigit(int c);
// 将字符转换为小写
int tolower(int c);
// 将字符转换为大写
int toupper(int c);

#ifdef __cplusplus
}
#endif

#endif /* SIMPLEKERNEL_SRC_KERNEL_LIBC_INCLUDE_SK_CTYPE_H_ */
