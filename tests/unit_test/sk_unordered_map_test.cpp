/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_unordered_map"

#include <gtest/gtest.h>

#include <string>

TEST(SkUnorderedMapTest, DefaultConstructor) {
  sk_std::unordered_map<int, int> map;
  EXPECT_TRUE(map.empty());
  EXPECT_EQ(map.size(), 0);
}

TEST(SkUnorderedMapTest, Insert) {
  sk_std::unordered_map<int, int> map;

  auto result1 = map.insert({1, 10});
  EXPECT_TRUE(result1.second);
  EXPECT_EQ(result1.first->first, 1);
  EXPECT_EQ(result1.first->second, 10);
  EXPECT_EQ(map.size(), 1);

  auto result2 = map.insert({1, 20});
  EXPECT_FALSE(result2.second);
  EXPECT_EQ(result2.first->second, 10);
  EXPECT_EQ(map.size(), 1);

  map.insert({2, 20});
  EXPECT_EQ(map.size(), 2);
}

TEST(SkUnorderedMapTest, OperatorBracket) {
  sk_std::unordered_map<int, int> map;

  map[1] = 10;
  EXPECT_EQ(map[1], 10);
  EXPECT_EQ(map.size(), 1);

  map[1] = 20;
  EXPECT_EQ(map[1], 20);
  EXPECT_EQ(map.size(), 1);

  map[2] = 30;
  EXPECT_EQ(map.size(), 2);
  EXPECT_EQ(map[2], 30);
}

TEST(SkUnorderedMapTest, Find) {
  sk_std::unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;

  auto it1 = map.find(1);
  EXPECT_NE(it1, map.end());
  EXPECT_EQ(it1->first, 1);
  EXPECT_EQ(it1->second, 10);

  auto it2 = map.find(3);
  EXPECT_EQ(it2, map.end());
}

TEST(SkUnorderedMapTest, Contains) {
  sk_std::unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;

  EXPECT_TRUE(map.contains(1));
  EXPECT_TRUE(map.contains(2));
  EXPECT_FALSE(map.contains(3));
}

TEST(SkUnorderedMapTest, Count) {
  sk_std::unordered_map<int, int> map;
  map[1] = 10;

  EXPECT_EQ(map.count(1), 1);
  EXPECT_EQ(map.count(2), 0);
}

TEST(SkUnorderedMapTest, Erase) {
  sk_std::unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;
  map[3] = 30;

  EXPECT_EQ(map.erase(2), 1);
  EXPECT_EQ(map.size(), 2);
  EXPECT_FALSE(map.contains(2));

  EXPECT_EQ(map.erase(10), 0);
  EXPECT_EQ(map.size(), 2);
}

TEST(SkUnorderedMapTest, Clear) {
  sk_std::unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;

  map.clear();
  EXPECT_TRUE(map.empty());
  EXPECT_EQ(map.size(), 0);
}

TEST(SkUnorderedMapTest, Iterator) {
  sk_std::unordered_map<int, int> map;
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

  EXPECT_EQ(count, 3);
  EXPECT_EQ(sum_keys, 6);
  EXPECT_EQ(sum_values, 60);
}

TEST(SkUnorderedMapTest, RangeBasedFor) {
  sk_std::unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;
  map[3] = 30;

  int count = 0;
  for (const auto& pair : map) {
    ++count;
    EXPECT_EQ(pair.second, pair.first * 10);
  }

  EXPECT_EQ(count, 3);
}

TEST(SkUnorderedMapTest, CopyConstructor) {
  sk_std::unordered_map<int, int> map1;
  map1[1] = 10;
  map1[2] = 20;

  sk_std::unordered_map<int, int> map2(map1);
  EXPECT_EQ(map2.size(), 2);
  EXPECT_EQ(map2[1], 10);
  EXPECT_EQ(map2[2], 20);

  map1[1] = 100;
  EXPECT_EQ(map2[1], 10);
}

TEST(SkUnorderedMapTest, CopyAssignment) {
  sk_std::unordered_map<int, int> map1;
  map1[1] = 10;
  map1[2] = 20;

  sk_std::unordered_map<int, int> map2;
  map2 = map1;

  EXPECT_EQ(map2.size(), 2);
  EXPECT_EQ(map2[1], 10);
  EXPECT_EQ(map2[2], 20);
}

TEST(SkUnorderedMapTest, MoveConstructor) {
  sk_std::unordered_map<int, int> map1;
  map1[1] = 10;
  map1[2] = 20;

  sk_std::unordered_map<int, int> map2(std::move(map1));
  EXPECT_EQ(map2.size(), 2);
  EXPECT_EQ(map2[1], 10);
  EXPECT_EQ(map2[2], 20);
  EXPECT_TRUE(map1.empty());
}

TEST(SkUnorderedMapTest, MoveAssignment) {
  sk_std::unordered_map<int, int> map1;
  map1[1] = 10;
  map1[2] = 20;

  sk_std::unordered_map<int, int> map2;
  map2 = std::move(map1);

  EXPECT_EQ(map2.size(), 2);
  EXPECT_EQ(map2[1], 10);
  EXPECT_EQ(map2[2], 20);
  EXPECT_TRUE(map1.empty());
}

TEST(SkUnorderedMapTest, At) {
  sk_std::unordered_map<int, int> map;
  map[1] = 10;

  EXPECT_EQ(map.at(1), 10);

  // at() 对不存在的键返回默认值
  EXPECT_EQ(map.at(2), 0);
}

TEST(SkUnorderedMapTest, Emplace) {
  sk_std::unordered_map<int, int> map;

  auto result = map.emplace(1, 10);
  EXPECT_TRUE(result.second);
  EXPECT_EQ(result.first->first, 1);
  EXPECT_EQ(result.first->second, 10);
  EXPECT_EQ(map.size(), 1);
}

TEST(SkUnorderedMapTest, BucketInterface) {
  sk_std::unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;

  EXPECT_GT(map.bucket_count(), 0);
  EXPECT_LE(map.load_factor(), 1.0f);

  size_t bucket_idx = map.bucket(1);
  EXPECT_LT(bucket_idx, map.bucket_count());
}

TEST(SkUnorderedMapTest, Reserve) {
  sk_std::unordered_map<int, int> map;

  size_t initial_bucket_count = map.bucket_count();
  map.reserve(100);
  EXPECT_GE(map.bucket_count(), initial_bucket_count);
}

TEST(SkUnorderedMapTest, Rehash) {
  sk_std::unordered_map<int, int> map;
  map[1] = 10;
  map[2] = 20;

  map.rehash(50);
  EXPECT_GE(map.bucket_count(), 50);
  EXPECT_EQ(map.size(), 2);
  EXPECT_EQ(map[1], 10);
  EXPECT_EQ(map[2], 20);
}

TEST(SkUnorderedMapTest, LargeDataSet) {
  sk_std::unordered_map<int, int> map;

  for (int i = 0; i < 1000; ++i) {
    map[i] = i * 2;
  }

  EXPECT_EQ(map.size(), 1000);

  for (int i = 0; i < 1000; ++i) {
    EXPECT_EQ(map[i], i * 2);
  }

  for (int i = 0; i < 500; ++i) {
    map.erase(i);
  }

  EXPECT_EQ(map.size(), 500);

  for (int i = 500; i < 1000; ++i) {
    EXPECT_TRUE(map.contains(i));
  }

  for (int i = 0; i < 500; ++i) {
    EXPECT_FALSE(map.contains(i));
  }
}

TEST(SkUnorderedMapTest, PointerKey) {
  sk_std::unordered_map<int*, int> map;

  int a = 1, b = 2, c = 3;
  map[&a] = 10;
  map[&b] = 20;
  map[&c] = 30;

  EXPECT_EQ(map.size(), 3);
  EXPECT_EQ(map[&a], 10);
  EXPECT_EQ(map[&b], 20);
  EXPECT_EQ(map[&c], 30);

  EXPECT_TRUE(map.contains(&a));
  EXPECT_FALSE(map.contains((int*)0x12345678));
}
