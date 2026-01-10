/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_queue"

#include "sk_cstdio"
#include "system_test.h"

using namespace sk_std;

namespace {

bool test_push_pop() {
  queue<int> q;
  EXPECT_EQ(q.size(), 0, "Initial size should be 0");
  EXPECT_EQ(q.empty(), true, "Initial queue should be empty");

  q.push(1);
  EXPECT_EQ(q.size(), 1, "Size should be 1 after push");
  EXPECT_EQ(q.front(), 1, "Front should be 1");
  EXPECT_EQ(q.back(), 1, "Back should be 1");

  q.push(2);
  EXPECT_EQ(q.size(), 2, "Size should be 2 after push");
  EXPECT_EQ(q.front(), 1, "Front should be 1");
  EXPECT_EQ(q.back(), 2, "Back should be 2");

  q.pop();
  EXPECT_EQ(q.size(), 1, "Size should be 1 after pop");
  EXPECT_EQ(q.front(), 2, "Front should be 2");
  EXPECT_EQ(q.back(), 2, "Back should be 2");

  q.pop();
  EXPECT_EQ(q.size(), 0, "Size should be 0 after pop");
  EXPECT_EQ(q.empty(), true, "Queue should be empty");

  return true;
}

}  // namespace

bool sk_queue_test() {
  if (!test_push_pop()) {
    return false;
  }
  return true;
}
