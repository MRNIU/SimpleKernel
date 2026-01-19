/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_unordered_map"

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

bool test_insert_and_find() {
  unordered_map<int, int> map;
  EXPECT_EQ(map.size(), 0, "Initial size should be 0");
  EXPECT_EQ(map.empty(), true, "Initial map should be empty");

  map.insert({1, 10});
  EXPECT_EQ(map.size(), 1, "Size should be 1 after insert");
  EXPECT_EQ(map.contains(1), true, "Key 1 should exist");

  auto it = map.find(1);
  EXPECT_NE((size_t) & *it, (size_t) & *map.end(),
            "find(1) should not return end()");

  map.insert({2, 20});
  map.insert({3, 30});
  EXPECT_EQ(map.size(), 3, "Size should be 3 after insertions");

  return true;
}

bool test_operator_bracket() {
  unordered_map<int, int> map;

  map[1] = 10;
  EXPECT_EQ(map[1], 10, "map[1] should be 10");
  EXPECT_EQ(map.size(), 1, "Size should be 1");

  map[1] = 20;
  EXPECT_EQ(map[1], 20, "map[1] should be 20 after update");
  EXPECT_EQ(map.size(), 1, "Size should still be 1");

  map[2] = 30;
  EXPECT_EQ(map.size(), 2, "Size should be 2");
  EXPECT_EQ(map[2], 30, "map[2] should be 30");

  return true;
}

bool test_erase() {
  unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;
  map[3] = 30;

  size_t erased = map.erase(2);
  EXPECT_EQ(erased, 1, "erase should return 1");
  EXPECT_EQ(map.size(), 2, "Size should be 2 after erase");
  EXPECT_EQ(map.contains(2), false, "Key 2 should not exist");

  erased = map.erase(10);
  EXPECT_EQ(erased, 0, "erase non-existing key should return 0");
  EXPECT_EQ(map.size(), 2, "Size should still be 2");

  return true;
}

bool test_clear() {
  unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;

  map.clear();
  EXPECT_EQ(map.size(), 0, "Size should be 0 after clear");
  EXPECT_EQ(map.empty(), true, "Map should be empty after clear");

  return true;
}

bool test_iterator() {
  unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;
  map[3] = 30;

  int count = 0;
  int sum_keys = 0;
  int sum_values = 0;

  for (auto it = map.begin(); it != map.end(); ++it) {
    ++count;
    sum_keys += it->first;
    sum_values += it->second;
  }

  EXPECT_EQ(count, 3, "Iterator should visit 3 elements");
  EXPECT_EQ(sum_keys, 6, "Sum of keys should be 6");
  EXPECT_EQ(sum_values, 60, "Sum of values should be 60");

  return true;
}

bool test_copy() {
  unordered_map<int, int> map1;
  map1[1] = 10;
  map1[2] = 20;

  unordered_map<int, int> map2(map1);
  EXPECT_EQ(map2.size(), 2, "Copied map size should be 2");
  EXPECT_EQ(map2[1], 10, "map2[1] should be 10");
  EXPECT_EQ(map2[2], 20, "map2[2] should be 20");

  map1[1] = 100;
  EXPECT_EQ(map2[1], 10, "map2[1] should still be 10");

  return true;
}

bool test_move() {
  unordered_map<int, int> map1;
  map1[1] = 10;
  map1[2] = 20;

  unordered_map<int, int> map2(static_cast<unordered_map<int, int>&&>(map1));
  EXPECT_EQ(map2.size(), 2, "Moved map size should be 2");
  EXPECT_EQ(map2[1], 10, "map2[1] should be 10");
  EXPECT_EQ(map2[2], 20, "map2[2] should be 20");
  EXPECT_EQ(map1.empty(), true, "map1 should be empty after move");

  return true;
}

bool test_large_dataset() {
  unordered_map<int, int> map;

  // 插入 100 个元素
  for (int i = 0; i < 100; ++i) {
    map[i] = i * 2;
  }

  EXPECT_EQ(map.size(), 100, "Size should be 100");

  // 验证所有元素
  for (int i = 0; i < 100; ++i) {
    if (map[i] != i * 2) {
      sk_printf("Error: map[%d] should be %d, got %d\n", i, i * 2, map[i]);
      return false;
    }
  }

  // 删除一半
  for (int i = 0; i < 50; ++i) {
    map.erase(i);
  }

  EXPECT_EQ(map.size(), 50, "Size should be 50 after erasing half");

  // 验证剩余元素
  for (int i = 50; i < 100; ++i) {
    EXPECT_EQ(map.contains(i), true, "Key should exist");
  }

  for (int i = 0; i < 50; ++i) {
    EXPECT_EQ(map.contains(i), false, "Key should not exist");
  }

  return true;
}

bool test_pointer_key() {
  unordered_map<int*, int> map;

  int a = 1, b = 2, c = 3;
  map[&a] = 10;
  map[&b] = 20;
  map[&c] = 30;

  EXPECT_EQ(map.size(), 3, "Size should be 3");
  EXPECT_EQ(map[&a], 10, "map[&a] should be 10");
  EXPECT_EQ(map[&b], 20, "map[&b] should be 20");
  EXPECT_EQ(map[&c], 30, "map[&c] should be 30");

  EXPECT_EQ(map.contains(&a), true, "Key &a should exist");

  return true;
}

}  // namespace

auto sk_unordered_map_test() -> bool {
  sk_printf("sk_unordered_map_test: start\n");
  if (!test_insert_and_find()) return false;
  if (!test_operator_bracket()) return false;
  if (!test_erase()) return false;
  if (!test_clear()) return false;
  if (!test_iterator()) return false;
  if (!test_copy()) return false;
  if (!test_move()) return false;
  if (!test_large_dataset()) return false;
  if (!test_pointer_key()) return false;
  return true;
}
