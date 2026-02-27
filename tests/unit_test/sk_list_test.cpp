/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "kstd_list"

// Use static_list<T, N> which has built-in storage (no external pool required)
static constexpr size_t kListCapacity = 64;

struct MyData {
  int x;
  double y;

  bool operator==(const MyData& other) const {
    return x == other.x && y == other.y;
  }
};

TEST(SkListTest, DefaultConstructor) {
  kstd::static_list<int, kListCapacity> sk_list;
  EXPECT_TRUE(sk_list.empty());
  EXPECT_EQ(sk_list.size(), 0);
}

TEST(SkListTest, PushFront) {
  kstd::static_list<int, kListCapacity> sk_list;
  sk_list.push_front(1);
  EXPECT_EQ(sk_list.size(), 1);
  EXPECT_EQ(sk_list.front(), 1);
  EXPECT_EQ(sk_list.back(), 1);

  sk_list.push_front(2);
  EXPECT_EQ(sk_list.size(), 2);
  EXPECT_EQ(sk_list.front(), 2);
  EXPECT_EQ(sk_list.back(), 1);
}

TEST(SkListTest, PushBack) {
  kstd::static_list<int, kListCapacity> sk_list;
  sk_list.push_back(1);
  EXPECT_EQ(sk_list.size(), 1);
  EXPECT_EQ(sk_list.front(), 1);
  EXPECT_EQ(sk_list.back(), 1);

  sk_list.push_back(2);
  EXPECT_EQ(sk_list.size(), 2);
  EXPECT_EQ(sk_list.front(), 1);
  EXPECT_EQ(sk_list.back(), 2);
}

TEST(SkListTest, PopFront) {
  kstd::static_list<int, kListCapacity> sk_list;
  sk_list.push_back(1);
  sk_list.push_back(2);
  sk_list.pop_front();
  EXPECT_EQ(sk_list.size(), 1);
  EXPECT_EQ(sk_list.front(), 2);
  sk_list.pop_front();
  EXPECT_EQ(sk_list.size(), 0);
}

TEST(SkListTest, PopBack) {
  kstd::static_list<int, kListCapacity> sk_list;
  sk_list.push_back(1);
  sk_list.push_back(2);
  sk_list.pop_back();
  EXPECT_EQ(sk_list.size(), 1);
  EXPECT_EQ(sk_list.back(), 1);
  sk_list.pop_back();
  EXPECT_EQ(sk_list.size(), 0);
}

TEST(SkListTest, Insert) {
  kstd::static_list<int, kListCapacity> sk_list;
  sk_list.push_back(1);
  sk_list.push_back(3);

  auto it = sk_list.begin();
  ++it;
  sk_list.insert(it, 2);

  EXPECT_EQ(sk_list.size(), 3);
  int i = 1;
  for (const auto& item : sk_list) {
    EXPECT_EQ(item, i++);
  }
}

TEST(SkListTest, Erase) {
  kstd::static_list<int, kListCapacity> sk_list;
  sk_list.push_back(1);
  sk_list.push_back(2);
  sk_list.push_back(3);

  auto it = sk_list.begin();
  ++it;
  sk_list.erase(it);

  EXPECT_EQ(sk_list.size(), 2);
  EXPECT_EQ(sk_list.front(), 1);
  EXPECT_EQ(sk_list.back(), 3);
}

TEST(SkListTest, Clear) {
  kstd::static_list<int, kListCapacity> sk_list;
  sk_list.push_back(1);
  sk_list.push_back(2);
  sk_list.clear();
  EXPECT_EQ(sk_list.size(), 0);
  EXPECT_TRUE(sk_list.empty());
}

TEST(SkListTest, FloatList) {
  kstd::static_list<float, kListCapacity> list;
  list.push_back(1.1f);
  list.push_back(2.2f);

  EXPECT_EQ(list.size(), 2);
  EXPECT_FLOAT_EQ(list.front(), 1.1f);
  EXPECT_FLOAT_EQ(list.back(), 2.2f);

  list.pop_front();
  EXPECT_EQ(list.size(), 1);
  EXPECT_FLOAT_EQ(list.front(), 2.2f);
}

TEST(SkListTest, StructList) {
  kstd::static_list<MyData, kListCapacity> list;
  list.push_back({1, 1.1});
  list.push_back({2, 2.2});

  EXPECT_EQ(list.size(), 2);
  EXPECT_EQ(list.front().x, 1);
  EXPECT_DOUBLE_EQ(list.front().y, 1.1);
  EXPECT_EQ(list.back().x, 2);
  EXPECT_DOUBLE_EQ(list.back().y, 2.2);

  list.pop_back();
  EXPECT_EQ(list.size(), 1);
  EXPECT_EQ(list.front().x, 1);
}

TEST(SkListTest, EraseRange) {
  kstd::static_list<int, kListCapacity> sk_list;
  for (int i = 1; i <= 5; ++i) {
    sk_list.push_back(i);
  }

  auto first = sk_list.begin();
  ++first;  // points to 2
  auto last = first;
  ++last;
  ++last;
  ++last;  // points to 5

  sk_list.erase(first, last);  // erase 2, 3, 4
  EXPECT_EQ(sk_list.size(), 2);
  EXPECT_EQ(sk_list.front(), 1);
  EXPECT_EQ(sk_list.back(), 5);
}

TEST(SkListTest, Remove) {
  kstd::static_list<int, kListCapacity> sk_list;
  sk_list.push_back(1);
  sk_list.push_back(2);
  sk_list.push_back(2);
  sk_list.push_back(3);
  sk_list.push_back(2);

  sk_list.remove(2);
  EXPECT_EQ(sk_list.size(), 2);
  EXPECT_EQ(sk_list.front(), 1);
  EXPECT_EQ(sk_list.back(), 3);
}

TEST(SkListTest, RemoveIf) {
  kstd::static_list<int, kListCapacity> sk_list;
  for (int i = 1; i <= 10; ++i) {
    sk_list.push_back(i);
  }

  sk_list.remove_if([](int x) { return x % 2 == 0; });  // remove even numbers
  EXPECT_EQ(sk_list.size(), 5);

  int expected = 1;
  for (const auto& item : sk_list) {
    EXPECT_EQ(item, expected);
    expected += 2;
  }
}
