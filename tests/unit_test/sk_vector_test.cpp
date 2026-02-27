/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "kstd_vector"

TEST(SkVectorTest, DefaultConstructor) {
  kstd::vector<int> v;
  EXPECT_TRUE(v.empty());
  EXPECT_EQ(v.size(), 0);
  EXPECT_EQ(v.capacity(), 0);
}

TEST(SkVectorTest, PushBack) {
  kstd::vector<int> v;
  v.push_back(1);
  EXPECT_EQ(v.size(), 1);
  EXPECT_EQ(v.front(), 1);
  EXPECT_EQ(v.back(), 1);

  v.push_back(2);
  EXPECT_EQ(v.size(), 2);
  EXPECT_EQ(v.front(), 1);
  EXPECT_EQ(v.back(), 2);
  EXPECT_GE(v.capacity(), 2);
}

TEST(SkVectorTest, PopBack) {
  kstd::vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.pop_back();
  EXPECT_EQ(v.size(), 1);
  EXPECT_EQ(v.back(), 1);
  v.pop_back();
  EXPECT_TRUE(v.empty());
}

TEST(SkVectorTest, Resize) {
  kstd::vector<int> v;
  v.resize(5);
  EXPECT_EQ(v.size(), 5);
  for (size_t i = 0; i < 5; ++i) {
    EXPECT_EQ(v[i], 0);
  }

  v.resize(2);
  EXPECT_EQ(v.size(), 2);

  v.resize(4, 10);
  EXPECT_EQ(v.size(), 4);
  EXPECT_EQ(v[2], 10);
  EXPECT_EQ(v[3], 10);
}

TEST(SkVectorTest, Clear) {
  kstd::vector<int> v;
  v.push_back(1);
  v.clear();
  EXPECT_TRUE(v.empty());
  EXPECT_EQ(v.size(), 0);
}

TEST(SkVectorTest, Access) {
  kstd::vector<int> v;
  v.push_back(10);
  v.push_back(20);
  EXPECT_EQ(v[0], 10);
  EXPECT_EQ(v.at(1), 20);
}

TEST(SkVectorTest, CopyConstructor) {
  kstd::vector<int> v1;
  v1.push_back(1);
  v1.push_back(2);

  kstd::vector<int> v2(v1);
  EXPECT_EQ(v2.size(), 2);
  EXPECT_EQ(v2[0], 1);
  EXPECT_EQ(v2[1], 2);
}

TEST(SkVectorTest, AssignmentOperator) {
  kstd::vector<int> v1;
  v1.push_back(1);
  v1.push_back(2);

  kstd::vector<int> v2;
  v2 = v1;
  EXPECT_EQ(v2.size(), 2);
  EXPECT_EQ(v2[0], 1);
  EXPECT_EQ(v2[1], 2);
}

TEST(SkVectorTest, Iterator) {
  kstd::vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.push_back(3);

  int sum = 0;
  for (auto it = v.begin(); it != v.end(); ++it) {
    sum += *it;
  }
  EXPECT_EQ(sum, 6);
}

TEST(SkVectorTest, EraseSingle) {
  kstd::vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.push_back(3);
  v.push_back(4);

  auto it = v.begin() + 1;  // points to 2
  it = v.erase(it);
  EXPECT_EQ(v.size(), 3);
  EXPECT_EQ(*it, 3);  // iterator points to next element
  EXPECT_EQ(v[0], 1);
  EXPECT_EQ(v[1], 3);
  EXPECT_EQ(v[2], 4);
}

TEST(SkVectorTest, EraseRange) {
  kstd::vector<int> v;
  for (int i = 1; i <= 5; ++i) {
    v.push_back(i);
  }

  auto first = v.begin() + 1;  // points to 2
  auto last = v.begin() + 4;   // points to 5
  auto it = v.erase(first, last);
  EXPECT_EQ(v.size(), 2);
  EXPECT_EQ(*it, 5);
  EXPECT_EQ(v[0], 1);
  EXPECT_EQ(v[1], 5);
}

TEST(SkVectorTest, Remove) {
  kstd::vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.push_back(2);
  v.push_back(3);
  v.push_back(2);

  v.remove(2);
  EXPECT_EQ(v.size(), 2);
  EXPECT_EQ(v[0], 1);
  EXPECT_EQ(v[1], 3);
}

TEST(SkVectorTest, RemoveIf) {
  kstd::vector<int> v;
  for (int i = 1; i <= 10; ++i) {
    v.push_back(i);
  }

  v.remove_if([](int x) { return x % 2 == 0; });  // remove even numbers
  EXPECT_EQ(v.size(), 5);
  for (size_t i = 0; i < v.size(); ++i) {
    EXPECT_EQ(v[i], static_cast<int>(i * 2 + 1));  // 1, 3, 5, 7, 9
  }
}
