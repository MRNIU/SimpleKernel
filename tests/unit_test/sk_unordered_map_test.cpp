/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include <string>

#include "kstd_unordered_map"

// Use static_unordered_map<K, V, MaxNodes, MaxBuckets> which has built-in
// storage (no external pool/bucket buffer required)
static constexpr size_t kMapNodes = 256;
static constexpr size_t kMapBuckets = 64;

TEST(SkUnorderedMapTest, DefaultConstructor) {
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
  EXPECT_TRUE(map.empty());
  EXPECT_EQ(map.size(), 0);
}

TEST(SkUnorderedMapTest, Insert) {
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;

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
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;

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
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
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
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
  map[1] = 10;
  map[2] = 20;

  EXPECT_TRUE(map.contains(1));
  EXPECT_TRUE(map.contains(2));
  EXPECT_FALSE(map.contains(3));
}

TEST(SkUnorderedMapTest, Count) {
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
  map[1] = 10;

  EXPECT_EQ(map.count(1), 1);
  EXPECT_EQ(map.count(2), 0);
}

TEST(SkUnorderedMapTest, Erase) {
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
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
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
  map[1] = 10;
  map[2] = 20;

  map.clear();
  EXPECT_TRUE(map.empty());
  EXPECT_EQ(map.size(), 0);
}

TEST(SkUnorderedMapTest, Iterator) {
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
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
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
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

TEST(SkUnorderedMapTest, At) {
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
  map[1] = 10;

  EXPECT_EQ(map.at(1), 10);

  // at() 对不存在的键返回默认值
  EXPECT_EQ(map.at(2), 0);
}

TEST(SkUnorderedMapTest, Emplace) {
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;

  auto result = map.emplace(1, 10);
  EXPECT_TRUE(result.second);
  EXPECT_EQ(result.first->first, 1);
  EXPECT_EQ(result.first->second, 10);
  EXPECT_EQ(map.size(), 1);
}

TEST(SkUnorderedMapTest, BucketInterface) {
  kstd::static_unordered_map<int, int, kMapNodes, kMapBuckets> map;
  map[1] = 10;
  map[2] = 20;

  EXPECT_GT(map.bucket_count(), 0);
  EXPECT_LE(map.load_factor(), 1.0f);

  size_t bucket_idx = map.bucket(1);
  EXPECT_LT(bucket_idx, map.bucket_count());
}

TEST(SkUnorderedMapTest, LargeDataSet) {
  // Use larger capacity for this test
  kstd::static_unordered_map<int, int, 1024, 256> map;

  for (int i = 0; i < 200; ++i) {
    map[i] = i * 2;
  }

  EXPECT_EQ(map.size(), 200);

  for (int i = 0; i < 200; ++i) {
    EXPECT_EQ(map[i], i * 2);
  }

  for (int i = 0; i < 100; ++i) {
    map.erase(i);
  }

  EXPECT_EQ(map.size(), 100);

  for (int i = 100; i < 200; ++i) {
    EXPECT_TRUE(map.contains(i));
  }

  for (int i = 0; i < 100; ++i) {
    EXPECT_FALSE(map.contains(i));
  }
}

TEST(SkUnorderedMapTest, PointerKey) {
  kstd::static_unordered_map<int*, int, kMapNodes, kMapBuckets> map;

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
