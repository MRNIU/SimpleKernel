/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_priority_queue"

#include <gtest/gtest.h>

TEST(SkPriorityQueueTest, DefaultConstructor) {
  sk_std::priority_queue<int> pq;
  EXPECT_TRUE(pq.empty());
  EXPECT_EQ(pq.size(), 0);
}

TEST(SkPriorityQueueTest, PushAndTop) {
  sk_std::priority_queue<int> pq;
  pq.push(10);
  EXPECT_FALSE(pq.empty());
  EXPECT_EQ(pq.size(), 1);
  EXPECT_EQ(pq.top(), 10);

  pq.push(20);
  EXPECT_EQ(pq.size(), 2);
  EXPECT_EQ(pq.top(), 20);  // Max-heap: 20 should be top

  pq.push(5);
  EXPECT_EQ(pq.size(), 3);
  EXPECT_EQ(pq.top(), 20);  // Still 20
}

TEST(SkPriorityQueueTest, Pop) {
  sk_std::priority_queue<int> pq;
  pq.push(10);
  pq.push(20);
  pq.push(5);

  // Order should be 20, 10, 5
  EXPECT_EQ(pq.top(), 20);
  pq.pop();
  EXPECT_EQ(pq.size(), 2);
  EXPECT_EQ(pq.top(), 10);

  pq.pop();
  EXPECT_EQ(pq.size(), 1);
  EXPECT_EQ(pq.top(), 5);

  pq.pop();
  EXPECT_EQ(pq.size(), 0);
  EXPECT_TRUE(pq.empty());
}

TEST(SkPriorityQueueTest, MinHeap) {
  // Use greater comparison for min-heap behavior
  struct greater {
    bool operator()(const int& lhs, const int& rhs) const { return lhs > rhs; }
  };

  sk_std::priority_queue<int, sk_std::vector<int>, greater> pq;
  pq.push(10);
  pq.push(20);
  pq.push(5);

  // Order should be 5, 10, 20
  EXPECT_EQ(pq.top(), 5);
  pq.pop();

  EXPECT_EQ(pq.top(), 10);
  pq.pop();

  EXPECT_EQ(pq.top(), 20);
  pq.pop();
  EXPECT_TRUE(pq.empty());
}
