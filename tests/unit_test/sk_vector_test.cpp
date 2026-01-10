/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_vector"

#include <gtest/gtest.h>

TEST(SkVectorTest, DefaultConstructor) {
  sk_std::vector<int> v;
  EXPECT_TRUE(v.empty());
  EXPECT_EQ(v.size(), 0);
  EXPECT_EQ(v.capacity(), 0);
}

TEST(SkVectorTest, PushBack) {
  sk_std::vector<int> v;
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
  sk_std::vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.pop_back();
  EXPECT_EQ(v.size(), 1);
  EXPECT_EQ(v.back(), 1);
  v.pop_back();
  EXPECT_TRUE(v.empty());
}

TEST(SkVectorTest, Resize) {
  sk_std::vector<int> v;
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
  sk_std::vector<int> v;
  v.push_back(1);
  v.clear();
  EXPECT_TRUE(v.empty());
  EXPECT_EQ(v.size(), 0);
}

TEST(SkVectorTest, Access) {
  sk_std::vector<int> v;
  v.push_back(10);
  v.push_back(20);
  EXPECT_EQ(v[0], 10);
  EXPECT_EQ(v.at(1), 20);
}

TEST(SkVectorTest, CopyConstructor) {
  sk_std::vector<int> v1;
  v1.push_back(1);
  v1.push_back(2);

  sk_std::vector<int> v2(v1);
  EXPECT_EQ(v2.size(), 2);
  EXPECT_EQ(v2[0], 1);
  EXPECT_EQ(v2[1], 2);
}

TEST(SkVectorTest, AssignmentOperator) {
  sk_std::vector<int> v1;
  v1.push_back(1);
  v1.push_back(2);

  sk_std::vector<int> v2;
  v2 = v1;
  EXPECT_EQ(v2.size(), 2);
  EXPECT_EQ(v2[0], 1);
  EXPECT_EQ(v2[1], 2);
}

TEST(SkVectorTest, Iterator) {
  sk_std::vector<int> v;
  v.push_back(1);
  v.push_back(2);
  v.push_back(3);

  int sum = 0;
  for (auto it = v.begin(); it != v.end(); ++it) {
    sum += *it;
  }
  EXPECT_EQ(sum, 6);
}
