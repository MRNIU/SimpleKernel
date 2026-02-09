/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_vector"

#include <cpu_io.h>

#include <cstddef>
#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "sk_cstdio"
#include "sk_cstring"
#include "sk_libcxx.h"
#include "sk_stdlib.h"
#include "system_test.h"

using namespace sk_std;

namespace {

bool test_push_pop() {
  vector<int> v;
  EXPECT_EQ(v.size(), 0, "Initial size should be 0");
  EXPECT_EQ(v.empty(), true, "Initial vector should be empty");

  v.push_back(1);
  EXPECT_EQ(v.size(), 1, "Size should be 1 after push_back");
  EXPECT_EQ(v.back(), 1, "Back should be 1");
  EXPECT_EQ(v.front(), 1, "Front should be 1");

  v.push_back(2);
  EXPECT_EQ(v.size(), 2, "Size should be 2 after push_back");
  EXPECT_EQ(v.back(), 2, "Back should be 2");
  EXPECT_EQ(v.front(), 1, "Front should be 1");

  v.pop_back();
  EXPECT_EQ(v.size(), 1, "Size should be 1 after pop_back");
  EXPECT_EQ(v.back(), 1, "Back should be 1");

  v.pop_back();
  EXPECT_EQ(v.size(), 0, "Size should be 0 after pop_back");
  EXPECT_EQ(v.empty(), true, "Vector should be empty");

  return true;
}

bool test_resize() {
  vector<int> v;
  v.resize(5);
  EXPECT_EQ(v.size(), 5, "Size should be 5 after resize(5)");
  for (size_t i = 0; i < 5; ++i) {
    if (v[i] != 0) {
      sk_printf("Error: v[%ld] should be 0\n", i);
      return false;
    }
  }

  v.resize(2);
  EXPECT_EQ(v.size(), 2, "Size should be 2 after resize(2)");

  v.resize(4, 10);
  EXPECT_EQ(v.size(), 4, "Size should be 4 after resize(4, 10)");
  if (v[2] != 10 || v[3] != 10) {
    sk_printf("Error: New elements should be 10\n");
    return false;
  }

  return true;
}

bool test_clear() {
  vector<int> v;
  v.push_back(1);
  v.clear();
  EXPECT_EQ(v.size(), 0, "Size should be 0 after clear");
  EXPECT_EQ(v.empty(), true, "Vector should be empty after clear");
  return true;
}

bool test_access() {
  vector<int> v;
  v.push_back(10);
  v.push_back(20);
  EXPECT_EQ(v[0], 10, "v[0] should be 10");
  EXPECT_EQ(v[1], 20, "v[1] should be 20");
  return true;
}

bool test_iterator() {
  vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.push_back(3);

  int sum = 0;
  for (auto it = v.begin(); it != v.end(); ++it) {
    sum += *it;
  }
  EXPECT_EQ(sum, 6, "Sum should be 6");
  return true;
}

bool test_erase_single() {
  vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.push_back(3);
  v.push_back(4);

  auto it = v.begin() + 1;  // points to 2
  it = v.erase(it);
  EXPECT_EQ(v.size(), 3, "Size should be 3 after erase");
  EXPECT_EQ(*it, 3, "Iterator should point to next element");
  EXPECT_EQ(v[0], 1, "v[0] should be 1");
  EXPECT_EQ(v[1], 3, "v[1] should be 3");
  EXPECT_EQ(v[2], 4, "v[2] should be 4");

  sk_printf("sk_vector_test: erase_single passed\n");
  return true;
}

bool test_erase_range() {
  vector<int> v;
  for (int i = 1; i <= 5; ++i) {
    v.push_back(i);
  }

  auto first = v.begin() + 1;  // points to 2
  auto last = v.begin() + 4;   // points to 5
  auto it = v.erase(first, last);
  EXPECT_EQ(v.size(), 2, "Size should be 2 after erase range");
  EXPECT_EQ(*it, 5, "Iterator should point to 5");
  EXPECT_EQ(v[0], 1, "v[0] should be 1");
  EXPECT_EQ(v[1], 5, "v[1] should be 5");

  sk_printf("sk_vector_test: erase_range passed\n");
  return true;
}

bool test_remove() {
  vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.push_back(2);
  v.push_back(3);
  v.push_back(2);

  v.remove(2);
  EXPECT_EQ(v.size(), 2, "Size should be 2 after remove");
  EXPECT_EQ(v[0], 1, "v[0] should be 1");
  EXPECT_EQ(v[1], 3, "v[1] should be 3");

  sk_printf("sk_vector_test: remove passed\n");
  return true;
}

bool test_remove_if() {
  vector<int> v;
  for (int i = 1; i <= 10; ++i) {
    v.push_back(i);
  }

  v.remove_if([](int x) { return x % 2 == 0; });  // remove even numbers
  EXPECT_EQ(v.size(), 5, "Size should be 5 after remove_if");
  for (size_t i = 0; i < v.size(); ++i) {
    EXPECT_EQ(v[i], static_cast<int>(i * 2 + 1), "Odd numbers should remain");
  }

  sk_printf("sk_vector_test: remove_if passed\n");
  return true;
}

}  // namespace

auto sk_vector_test() -> bool {
  sk_printf("sk_vector_test: start\n");
  if (!test_push_pop()) return false;
  if (!test_resize()) return false;
  if (!test_clear()) return false;
  if (!test_access()) return false;
  if (!test_iterator()) return false;
  if (!test_erase_single()) return false;
  if (!test_erase_range()) return false;
  if (!test_remove()) return false;
  if (!test_remove_if()) return false;
  return true;
}
