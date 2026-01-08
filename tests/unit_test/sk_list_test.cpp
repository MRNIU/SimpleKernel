/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_list"

#include <gtest/gtest.h>

struct MyData {
  int x;
  double y;

  bool operator==(const MyData& other) const {
    return x == other.x && y == other.y;
  }
};

TEST(SkListTest, DefaultConstructor) {
  sk_std::list<int> sk_list;
  EXPECT_TRUE(sk_list.empty());
  EXPECT_EQ(sk_list.size(), 0);
}

TEST(SkListTest, PushFront) {
  sk_std::list<int> sk_list;
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
  sk_std::list<int> sk_list;
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
  sk_std::list<int> sk_list;
  sk_list.push_back(1);
  sk_list.push_back(2);
  sk_list.pop_front();
  EXPECT_EQ(sk_list.size(), 1);
  EXPECT_EQ(sk_list.front(), 2);
  sk_list.pop_front();
  EXPECT_EQ(sk_list.size(), 0);
}

TEST(SkListTest, PopBack) {
  sk_std::list<int> sk_list;
  sk_list.push_back(1);
  sk_list.push_back(2);
  sk_list.pop_back();
  EXPECT_EQ(sk_list.size(), 1);
  EXPECT_EQ(sk_list.back(), 1);
  sk_list.pop_back();
  EXPECT_EQ(sk_list.size(), 0);
}

TEST(SkListTest, Insert) {
  sk_std::list<int> sk_list;
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
  sk_std::list<int> sk_list;
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
  sk_std::list<int> sk_list;
  sk_list.push_back(1);
  sk_list.push_back(2);
  sk_list.clear();
  EXPECT_EQ(sk_list.size(), 0);
  EXPECT_TRUE(sk_list.empty());
}

TEST(SkListTest, CopyConstructor) {
  sk_std::list<int> l1;
  l1.push_back(1);
  l1.push_back(2);

  sk_std::list<int> l2(l1);
  EXPECT_EQ(l2.size(), 2);
  EXPECT_EQ(l2.front(), 1);
  EXPECT_EQ(l2.back(), 2);

  l1.pop_back();
  EXPECT_EQ(l1.size(), 1);
  EXPECT_EQ(l2.size(), 2);
}

TEST(SkListTest, AssignmentOperator) {
  sk_std::list<int> l1;
  l1.push_back(1);
  l1.push_back(2);

  sk_std::list<int> l2;
  l2 = l1;
  EXPECT_EQ(l2.size(), 2);
  EXPECT_EQ(l2.front(), 1);
  EXPECT_EQ(l2.back(), 2);

  l1.pop_back();
  EXPECT_EQ(l1.size(), 1);
  EXPECT_EQ(l2.size(), 2);
}

TEST(SkListTest, FloatList) {
  sk_std::list<float> list;
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
  sk_std::list<MyData> list;
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
