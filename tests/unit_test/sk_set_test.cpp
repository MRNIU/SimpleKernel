/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_set"

#include <gtest/gtest.h>

namespace sk_std {
namespace {

TEST(SetTest, BasicOperations) {
  sk_std::set<int> s;
  EXPECT_TRUE(s.empty());
  EXPECT_EQ(s.size(), 0);

  auto result = s.insert(10);
  EXPECT_TRUE(result.second);
  EXPECT_EQ(*result.first, 10);
  EXPECT_EQ(s.size(), 1);

  result = s.insert(10);
  EXPECT_FALSE(result.second);
  EXPECT_EQ(*result.first, 10);
  EXPECT_EQ(s.size(), 1);

  s.insert(5);
  s.insert(15);
  EXPECT_EQ(s.size(), 3);

  auto it = s.find(5);
  EXPECT_NE(it, s.end());
  EXPECT_EQ(*it, 5);

  it = s.find(100);
  EXPECT_EQ(it, s.end());
}

TEST(SetTest, Iteration) {
  sk_std::set<int> s;
  s.insert(20);
  s.insert(10);
  s.insert(30);

  auto it = s.begin();
  EXPECT_EQ(*it, 10);
  ++it;
  EXPECT_EQ(*it, 20);
  ++it;
  EXPECT_EQ(*it, 30);
  ++it;
  EXPECT_EQ(it, s.end());
}

TEST(SetTest, Delete) {
  sk_std::set<int> s;
  s.insert(10);
  s.insert(5);
  s.insert(15);
  s.insert(8);
  s.insert(12);

  s.erase(5);  // Erase leaf/simple
  EXPECT_EQ(s.size(), 4);
  EXPECT_EQ(s.find(5), s.end());

  auto it = s.find(10);
  s.erase(it);  // Erase root/internal with 2 children
  EXPECT_EQ(s.size(), 3);
  EXPECT_EQ(s.find(10), s.end());

  EXPECT_EQ(*s.begin(), 8);
}

TEST(SetTest, CopyAndAssign) {
  sk_std::set<int> s1;
  s1.insert(1);
  s1.insert(2);

  sk_std::set<int> s2(s1);
  EXPECT_EQ(s2.size(), 2);
  // iterators should point to different nodes so find results should differ in
  // pointer value if checked (but we check value logic) Modify s1, s2 should be
  // intact
  s1.erase(1);
  EXPECT_EQ(s1.size(), 1);
  EXPECT_EQ(s2.size(), 2);

  sk_std::set<int> s3;
  s3 = s2;
  EXPECT_EQ(s3.size(), 2);
  EXPECT_TRUE(s3.find(1) != s3.end());

  s2.erase(2);
  EXPECT_EQ(s3.size(), 2);
}

TEST(SetTest, Range) {
  sk_std::set<int> s;
  s.insert(10);
  s.insert(20);
  s.insert(30);

  auto it = s.lower_bound(15);
  EXPECT_EQ(*it, 20);

  it = s.upper_bound(20);
  EXPECT_EQ(*it, 30);

  it = s.upper_bound(30);
  EXPECT_EQ(it, s.end());
}

}  // namespace
}  // namespace sk_std
