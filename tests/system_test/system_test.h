/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_TESTS_SYSTEM_TEST_SYSTEM_TEST_H_
#define SIMPLEKERNEL_TESTS_SYSTEM_TEST_SYSTEM_TEST_H_

#include <type_traits>

#include "sk_cstdio"

template <typename T1, typename T2>
bool expect_eq_helper(const T1 &val1, const T2 &val2, const char *msg) {
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-compare"
  if (val1 != val2) {
#pragma GCC diagnostic pop
    if constexpr (std::is_convertible_v<T1, long> &&
                  std::is_convertible_v<T2, long>) {
      sk_printf("FAIL: %s. Expected %ld, got %ld\n", msg, (long)(val2),
                (long)(val1));
    } else {
      sk_printf("FAIL: %s.\n", msg);
    }
    return false;
  }
  return true;
}

template <typename T1, typename T2>
bool expect_ne_helper(const T1 &val1, const T2 &val2, const char *msg) {
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-compare"
  if (val1 == val2) {
#pragma GCC diagnostic pop
    if constexpr (std::is_convertible_v<T1, long> &&
                  std::is_convertible_v<T2, long>) {
      sk_printf("FAIL: %s. Expected not %ld, got %ld\n", msg, (long)(val2),
                (long)(val1));
    } else {
      sk_printf("FAIL: %s.\n", msg);
    }
    return false;
  }
  return true;
}

#define EXPECT_EQ(val1, val2, msg)          \
  if (!expect_eq_helper(val1, val2, msg)) { \
    return false;                           \
  }

#define EXPECT_NE(val1, val2, msg)          \
  if (!expect_ne_helper(val1, val2, msg)) { \
    return false;                           \
  }

#define EXPECT_TRUE(cond, msg)     \
  if (!(cond)) {                   \
    sk_printf("FAIL: %s.\n", msg); \
    return false;                  \
  }

auto ctor_dtor_test() -> bool;
auto spinlock_test() -> bool;
auto memory_test() -> bool;
auto virtual_memory_test() -> bool;
auto interrupt_test() -> bool;
auto sk_list_test() -> bool;
auto sk_queue_test() -> bool;
auto sk_vector_test() -> bool;
auto sk_priority_queue_test() -> bool;
auto sk_rb_tree_test() -> bool;
auto sk_set_test() -> bool;
auto sk_unordered_map_test() -> bool;

#endif /* SIMPLEKERNEL_TESTS_SYSTEM_TEST_SYSTEM_TEST_H_ */
