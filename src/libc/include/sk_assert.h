/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_ASSERT_H_
#define SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_ASSERT_H_

#include <cpu_io.h>

#include "kernel_log.hpp"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief 运行时断言宏
 * @param expr 断言表达式
 * @note 断言失败时打印错误信息并暂停 CPU
 */
#define sk_assert(expr)                                                       \
  do {                                                                        \
    if (!(expr)) {                                                            \
      klog::Err("\n[ASSERT FAILED] %s:%d in %s\n Expression: %s\n", __FILE__, \
                __LINE__, __PRETTY_FUNCTION__, #expr);                        \
      while (true) {                                                          \
        cpu_io::Pause();                                                      \
      }                                                                       \
    }                                                                         \
  } while (0)

/**
 * @brief 带自定义消息的运行时断言宏（支持变长参数）
 * @param expr 断言表达式
 * @param fmt 格式化字符串
 * @param ... 格式化参数
 */
#define sk_assert_msg(expr, fmt, ...)                                      \
  do {                                                                     \
    if (!(expr)) {                                                         \
      klog::Err(                                                           \
          "\n[ASSERT FAILED] %s:%d in %s\n Expression: %s\n Message: " fmt \
          "\n",                                                            \
          __FILE__, __LINE__, __PRETTY_FUNCTION__, #expr, ##__VA_ARGS__);  \
      while (true) {                                                       \
        cpu_io::Pause();                                                   \
      }                                                                    \
    }                                                                      \
  } while (0)

#ifdef __cplusplus
}
#endif

#endif /* SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_ASSERT_H_ */
