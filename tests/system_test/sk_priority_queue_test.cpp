/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_priority_queue"

#include "sk_cstdio"
#include "system_test.h"

using namespace sk_std;

namespace {

bool test_max_heap() {
  priority_queue<int> pq;
  EXPECT_EQ(pq.size(), 0, "Initial size should be 0");
  EXPECT_TRUE(pq.empty(), "Initial queue should be empty");

  pq.push(10);
  EXPECT_EQ(pq.size(), 1, "Size should be 1 after push");
  EXPECT_EQ(pq.top(), 10, "Top should be 10");

  pq.push(20);
  EXPECT_EQ(pq.size(), 2, "Size should be 2 after push");
  EXPECT_EQ(pq.top(), 20, "Top should be 20");

  pq.push(5);
  EXPECT_EQ(pq.size(), 3, "Size should be 3 after push");
  EXPECT_EQ(pq.top(), 20, "Top should be 20");

  pq.pop();
  EXPECT_EQ(pq.size(), 2, "Size should be 2 after pop");
  EXPECT_EQ(pq.top(), 10, "Top should be 10");

  pq.pop();
  EXPECT_EQ(pq.size(), 1, "Size should be 1 after pop");
  EXPECT_EQ(pq.top(), 5, "Top should be 5");

  pq.pop();
  EXPECT_EQ(pq.size(), 0, "Size should be 0 after pop");
  EXPECT_TRUE(pq.empty(), "Queue should be empty");

  return true;
}

struct greater {
  bool operator()(const int& lhs, const int& rhs) const { return lhs > rhs; }
};

bool test_min_heap() {
  priority_queue<int, vector<int>, greater> pq;
  pq.push(10);
  pq.push(20);
  pq.push(5);

  EXPECT_EQ(pq.top(), 5, "Min heap top should be 5");
  pq.pop();

  EXPECT_EQ(pq.top(), 10, "Min heap top should be 10");
  pq.pop();

  EXPECT_EQ(pq.top(), 20, "Min heap top should be 20");
  pq.pop();

  EXPECT_TRUE(pq.empty(), "Min heap should be empty");
  return true;
}

}  // namespace

bool sk_priority_queue_test() {
  if (!test_max_heap()) {
    sk_printf("SkPriorityQueueTest: test_max_heap failed\n");
    return false;
  }
  if (!test_min_heap()) {
    sk_printf("SkPriorityQueueTest: test_min_heap failed\n");
    return false;
  }
  return true;
}
