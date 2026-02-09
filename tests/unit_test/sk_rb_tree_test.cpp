/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_rb_tree"

#include <gtest/gtest.h>

#include <functional>

namespace sk_std {
namespace {

// Test using RbTree directly as a set (similar to sk_set implementation)
template <typename T>
struct Identity {
  const T& operator()(const T& x) const { return x; }
};

using IntTree = RbTree<int, int, Identity<int>, std::less<int>>;

TEST(RbTreeTest, BasicOperations) {
  IntTree tree;
  EXPECT_TRUE(tree.empty());
  EXPECT_EQ(tree.size(), 0);

  tree.insert_unique(10);
  EXPECT_FALSE(tree.empty());
  EXPECT_EQ(tree.size(), 1);
  EXPECT_EQ(*tree.begin(), 10);

  tree.insert_unique(5);
  tree.insert_unique(15);
  EXPECT_EQ(tree.size(), 3);

  auto it = tree.find(5);
  EXPECT_NE(it, tree.end());
  EXPECT_EQ(*it, 5);

  it = tree.find(20);
  EXPECT_EQ(it, tree.end());
}

TEST(RbTreeTest, CopyConstructor) {
  IntTree tree1;
  tree1.insert_unique(1);
  tree1.insert_unique(2);
  tree1.insert_unique(3);

  IntTree tree2(tree1);
  EXPECT_EQ(tree2.size(), 3);
  EXPECT_NE(tree2.find(2), tree2.end());
}

TEST(RbTreeTest, AssignmentOperator) {
  IntTree tree1;
  tree1.insert_unique(10);
  tree1.insert_unique(20);

  IntTree tree2;
  tree2.insert_unique(30);

  tree2 = tree1;
  EXPECT_EQ(tree2.size(), 2);
  EXPECT_NE(tree2.find(10), tree2.end());
  EXPECT_EQ(tree2.find(30), tree2.end());
}

TEST(RbTreeTest, Clear) {
  IntTree tree;
  for (int i = 0; i < 10; ++i) {
    tree.insert_unique(i);
  }
  EXPECT_EQ(tree.size(), 10);
  tree.clear();
  EXPECT_EQ(tree.size(), 0);
  EXPECT_TRUE(tree.empty());
}

TEST(RbTreeTest, LowerUpperBound) {
  IntTree tree;
  tree.insert_unique(10);
  tree.insert_unique(20);
  tree.insert_unique(30);

  EXPECT_EQ(*tree.lower_bound(15), 20);
  EXPECT_EQ(*tree.upper_bound(20), 30);
  EXPECT_EQ(tree.upper_bound(30), tree.end());
}

TEST(RbTreeTest, Erase) {
  IntTree tree;
  tree.insert_unique(10);
  tree.insert_unique(5);
  tree.insert_unique(15);

  EXPECT_EQ(tree.size(), 3);

  // Test erase by key
  EXPECT_EQ(tree.erase(5), 1);
  EXPECT_EQ(tree.size(), 2);
  EXPECT_EQ(tree.find(5), tree.end());

  // Test erase by iterator
  auto it = tree.find(15);
  tree.erase(it);
  EXPECT_EQ(tree.size(), 1);
  EXPECT_EQ(tree.find(15), tree.end());
}

TEST(RbTreeTest, IteratorTraversal) {
  IntTree tree;
  tree.insert_unique(20);
  tree.insert_unique(10);
  tree.insert_unique(30);

  // In-order traversal validation
  auto it = tree.begin();
  EXPECT_EQ(*it, 10);
  ++it;
  EXPECT_EQ(*it, 20);
  ++it;
  EXPECT_EQ(*it, 30);
  ++it;
  EXPECT_EQ(it, tree.end());

  --it;
  EXPECT_EQ(*it, 30);
  --it;
  EXPECT_EQ(*it, 20);
}

// Simple logic to verify black height property (simplified)
// Since we don't expose tree structure, just rely on correctness of insertion
// not crashing and maintaining order is usually enough for black-box testing,
// but we can iterate.

}  // namespace
}  // namespace sk_std
