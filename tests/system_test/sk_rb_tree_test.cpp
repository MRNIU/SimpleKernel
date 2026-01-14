/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_rb_tree"

#include "system_test.h"

using namespace sk_std;

namespace {

template <typename T>
struct Identity {
  const T& operator()(const T& x) const { return x; }
};

using IntTree = RbTree<int, int, Identity<int>, std::less<int>>;

bool test_basic_operations() {
  IntTree tree;
  EXPECT_EQ(tree.empty(), true, "Tree should be empty initially");
  EXPECT_EQ(tree.size(), 0, "Tree size should be 0");

  tree.insert_unique(10);
  EXPECT_EQ(tree.empty(), false, "Tree should not be empty");
  EXPECT_EQ(tree.size(), 1, "Tree size should be 1");
  EXPECT_EQ(*tree.begin(), 10, "Begin should be 10");

  tree.insert_unique(5);
  tree.insert_unique(15);
  EXPECT_EQ(tree.size(), 3, "Tree size should be 3");

  auto it = tree.find(5);
  EXPECT_NE(it, tree.end(), "Should find 5");
  EXPECT_EQ(*it, 5, "Found value should be 5");

  it = tree.find(20);
  EXPECT_EQ(it, tree.end(), "Should not find 20");

  return true;
}

bool test_erase() {
  IntTree tree;
  tree.insert_unique(10);
  tree.insert_unique(5);
  tree.insert_unique(15);

  tree.erase(5);
  EXPECT_EQ(tree.size(), 2, "Size should be 2 after erase");
  EXPECT_EQ(tree.find(5), tree.end(), "Should not find 5");

  return true;
}

}  // namespace

auto sk_rb_tree_test() -> bool {
  if (!test_basic_operations()) return false;
  if (!test_erase()) return false;
  return true;
}
