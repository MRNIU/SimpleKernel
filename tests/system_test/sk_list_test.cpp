/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_list"

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
  list<int> l;
  EXPECT_EQ(l.size(), 0, "Initial size should be 0");
  EXPECT_EQ(l.empty(), true, "Initial list should be empty");

  l.push_back(1);
  EXPECT_EQ(l.size(), 1, "Size should be 1 after push_back");
  EXPECT_EQ(l.back(), 1, "Back should be 1");
  EXPECT_EQ(l.front(), 1, "Front should be 1");

  l.push_back(2);
  EXPECT_EQ(l.size(), 2, "Size should be 2 after push_back");
  EXPECT_EQ(l.back(), 2, "Back should be 2");
  EXPECT_EQ(l.front(), 1, "Front should be 1");

  l.push_front(0);
  EXPECT_EQ(l.size(), 3, "Size should be 3 after push_front");
  EXPECT_EQ(l.front(), 0, "Front should be 0");
  EXPECT_EQ(l.back(), 2, "Back should be 2");

  l.pop_back();
  EXPECT_EQ(l.size(), 2, "Size should be 2 after pop_back");
  EXPECT_EQ(l.back(), 1, "Back should be 1");

  l.pop_front();
  EXPECT_EQ(l.size(), 1, "Size should be 1 after pop_front");
  EXPECT_EQ(l.front(), 1, "Front should be 1");

  l.clear();
  EXPECT_EQ(l.size(), 0, "Size should be 0 after clear");
  EXPECT_EQ(l.empty(), true, "List should be empty after clear");

  sk_printf("sk_list_test: push_pop passed\n");
  return true;
}

bool test_iterator() {
  list<int> l;
  l.push_back(10);
  l.push_back(20);
  l.push_back(30);

  auto it = l.begin();
  EXPECT_EQ(*it, 10, "First element should be 10");
  ++it;
  EXPECT_EQ(*it, 20, "Second element should be 20");
  it++;
  EXPECT_EQ(*it, 30, "Third element should be 30");
  ++it;
  EXPECT_EQ(it == l.end(), true, "Iterator should be at end");

  sk_printf("sk_list_test: iterator passed\n");
  return true;
}

bool test_copy() {
  list<int> l1;
  l1.push_back(1);
  l1.push_back(2);

  list<int> l2(l1);
  EXPECT_EQ(l2.size(), 2, "Copied list size");
  EXPECT_EQ(l2.front(), 1, "Copied list front");
  EXPECT_EQ(l2.back(), 2, "Copied list back");

  list<int> l3;
  l3 = l1;
  EXPECT_EQ(l3.size(), 2, "Assigned list size");
  EXPECT_EQ(l3.front(), 1, "Assigned list front");
  EXPECT_EQ(l3.back(), 2, "Assigned list back");

  sk_printf("sk_list_test: copy passed\n");
  return true;
}

bool test_insert_erase() {
  list<int> l;
  l.push_back(1);
  l.push_back(3);

  auto it = l.begin();
  ++it;             // points to 3
  l.insert(it, 2);  // insert 2 before 3

  EXPECT_EQ(l.size(), 3, "Size after insert");

  it = l.begin();
  EXPECT_EQ(*it, 1, "1st element");
  ++it;
  EXPECT_EQ(*it, 2, "2nd element");
  ++it;
  EXPECT_EQ(*it, 3, "3rd element");

  it = l.begin();
  ++it;  // points to 2
  l.erase(it);

  EXPECT_EQ(l.size(), 2, "Size after erase");
  EXPECT_EQ(l.front(), 1, "Front after erase");
  EXPECT_EQ(l.back(), 3, "Back after erase");

  sk_printf("sk_list_test: insert_erase passed\n");
  return true;
}

struct TestData {
  int x;
  int y;

  bool operator==(const TestData& other) const {
    return x == other.x && y == other.y;
  }
  bool operator!=(const TestData& other) const { return !(*this == other); }
};

bool test_struct_type() {
  list<TestData> l;
  l.push_back({1, 2});
  l.push_back({3, 4});

  EXPECT_EQ(l.size(), 2, "Struct list size");

  TestData& front = l.front();
  EXPECT_EQ(front.x, 1, "Struct front.x");
  EXPECT_EQ(front.y, 2, "Struct front.y");

  TestData& back = l.back();
  EXPECT_EQ(back.x, 3, "Struct back.x");
  EXPECT_EQ(back.y, 4, "Struct back.y");

  sk_printf("sk_list_test: struct_type passed\n");
  return true;
}

bool test_char_type() {
  list<char> l;
  l.push_back('a');
  l.push_back('b');

  EXPECT_EQ(l.size(), 2, "Char list size");
  EXPECT_EQ(l.front(), 'a', "Char list front");
  EXPECT_EQ(l.back(), 'b', "Char list back");

  sk_printf("sk_list_test: char_type passed\n");
  return true;
}

bool test_erase_range() {
  list<int> l;
  for (int i = 1; i <= 5; ++i) {
    l.push_back(i);
  }

  auto first = l.begin();
  ++first;  // points to 2
  auto last = first;
  ++last;
  ++last;
  ++last;  // points to 5

  l.erase(first, last);  // erase 2, 3, 4
  EXPECT_EQ(l.size(), 2, "Size should be 2 after erase range");
  EXPECT_EQ(l.front(), 1, "Front should be 1");
  EXPECT_EQ(l.back(), 5, "Back should be 5");

  sk_printf("sk_list_test: erase_range passed\n");
  return true;
}

bool test_remove() {
  list<int> l;
  l.push_back(1);
  l.push_back(2);
  l.push_back(2);
  l.push_back(3);
  l.push_back(2);

  l.remove(2);
  EXPECT_EQ(l.size(), 2, "Size should be 2 after remove");
  EXPECT_EQ(l.front(), 1, "Front should be 1");
  EXPECT_EQ(l.back(), 3, "Back should be 3");

  sk_printf("sk_list_test: remove passed\n");
  return true;
}

bool test_remove_if() {
  list<int> l;
  for (int i = 1; i <= 10; ++i) {
    l.push_back(i);
  }

  l.remove_if([](int x) { return x % 2 == 0; });  // remove even numbers
  EXPECT_EQ(l.size(), 5, "Size should be 5 after remove_if");

  int expected = 1;
  for (const auto& item : l) {
    EXPECT_EQ(item, expected, "Should contain only odd numbers");
    expected += 2;
  }

  sk_printf("sk_list_test: remove_if passed\n");
  return true;
}

}  // namespace

auto sk_list_test() -> bool {
  sk_printf("sk_list_test: start\n");
  if (!test_push_pop()) return false;
  if (!test_iterator()) return false;
  if (!test_copy()) return false;
  if (!test_insert_erase()) return false;
  if (!test_struct_type()) return false;
  if (!test_char_type()) return false;
  if (!test_erase_range()) return false;
  if (!test_remove()) return false;
  if (!test_remove_if()) return false;
  return true;
}
