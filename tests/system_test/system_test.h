/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_TESTS_SYSTEM_TEST_SYSTEM_TEST_H_
#define SIMPLEKERNEL_TESTS_SYSTEM_TEST_SYSTEM_TEST_H_

#include "sk_cstdio"

#define EXPECT_EQ(val1, val2, msg)                                    \
  if ((val1) != (val2)) {                                             \
    sk_printf("FAIL: %s. Expected %ld, got %ld\n", msg, (long)(val2), \
              (long)(val1));                                          \
    return false;                                                     \
  }

#define EXPECT_NE(val1, val2, msg)                                        \
  if ((val1) == (val2)) {                                                 \
    sk_printf("FAIL: %s. Expected not %ld, got %ld\n", msg, (long)(val2), \
              (long)(val1));                                              \
    return false;                                                         \
  }

#define EXPECT_TRUE(cond, msg)     \
  if (!(cond)) {                   \
    sk_printf("FAIL: %s.\n", msg); \
    return false;                  \
  }

auto ctor_dtor_test() -> bool;
auto spinlock_test() -> bool;
auto memory_test() -> bool;
auto sk_list_test() -> bool;
auto sk_queue_test() -> bool;
auto sk_vector_test() -> bool;
auto kernel_task_test() -> bool;
auto user_task_test() -> bool;

#endif /* SIMPLEKERNEL_TESTS_SYSTEM_TEST_SYSTEM_TEST_H_ */
