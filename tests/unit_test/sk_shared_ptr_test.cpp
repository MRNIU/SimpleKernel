/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_shared_ptr"

#include <gtest/gtest.h>

namespace {

struct TestObj {
  int value;
  static int destroy_count;
  explicit TestObj(int v) : value(v) {}
  ~TestObj() { ++destroy_count; }
};
int TestObj::destroy_count = 0;

class SharedPtrTest : public ::testing::Test {
 protected:
  void SetUp() override { TestObj::destroy_count = 0; }
};

// 1. Default construction — null, use_count == 0
TEST_F(SharedPtrTest, DefaultConstruction) {
  sk_std::shared_ptr<TestObj> p;
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(p.use_count(), 0u);
  EXPECT_FALSE(static_cast<bool>(p));
}

// 2. Construction from raw pointer — non-null, use_count == 1
TEST_F(SharedPtrTest, ConstructionFromRawPointer) {
  sk_std::shared_ptr<TestObj> p(new TestObj(42));
  EXPECT_NE(p.get(), nullptr);
  EXPECT_EQ(p.use_count(), 1u);
  EXPECT_TRUE(static_cast<bool>(p));
  EXPECT_EQ(p->value, 42);
}

// 3. Copy construction — both point to same object, use_count == 2
TEST_F(SharedPtrTest, CopyConstruction) {
  sk_std::shared_ptr<TestObj> p1(new TestObj(10));
  sk_std::shared_ptr<TestObj> p2(
      p1);  // NOLINT(performance-unnecessary-copy-initialization)
  EXPECT_EQ(p1.get(), p2.get());
  EXPECT_EQ(p1.use_count(), 2u);
  EXPECT_EQ(p2.use_count(), 2u);
  EXPECT_EQ(TestObj::destroy_count, 0);
}

// 4. Copy assignment — correct ref counting
TEST_F(SharedPtrTest, CopyAssignment) {
  sk_std::shared_ptr<TestObj> p1(new TestObj(1));
  sk_std::shared_ptr<TestObj> p2(new TestObj(2));
  EXPECT_EQ(p1.use_count(), 1u);
  EXPECT_EQ(p2.use_count(), 1u);

  p2 = p1;                               // p2's old object should be destroyed
  EXPECT_EQ(TestObj::destroy_count, 1);  // old p2 object deleted
  EXPECT_EQ(p1.get(), p2.get());
  EXPECT_EQ(p1.use_count(), 2u);
  EXPECT_EQ(p2.use_count(), 2u);
}

// 5. Move construction — source becomes null, target gets ownership
TEST_F(SharedPtrTest, MoveConstruction) {
  sk_std::shared_ptr<TestObj> p1(new TestObj(99));
  TestObj* raw = p1.get();

  sk_std::shared_ptr<TestObj> p2(
      static_cast<sk_std::shared_ptr<TestObj>&&>(p1));
  EXPECT_EQ(p1.get(), nullptr);
  EXPECT_EQ(p1.use_count(), 0u);
  EXPECT_EQ(p2.get(), raw);
  EXPECT_EQ(p2.use_count(), 1u);
  EXPECT_EQ(TestObj::destroy_count, 0);
}

// 6. Move assignment — correct transfer
TEST_F(SharedPtrTest, MoveAssignment) {
  sk_std::shared_ptr<TestObj> p1(new TestObj(7));
  sk_std::shared_ptr<TestObj> p2(new TestObj(8));
  TestObj* raw1 = p1.get();

  p2 = static_cast<sk_std::shared_ptr<TestObj>&&>(p1);
  EXPECT_EQ(TestObj::destroy_count, 1);  // p2's old object destroyed
  EXPECT_EQ(p1.get(), nullptr);
  EXPECT_EQ(p2.get(), raw1);
  EXPECT_EQ(p2.use_count(), 1u);
}

// 7. Destructor — object deleted when last shared_ptr destroyed
TEST_F(SharedPtrTest, DestructorDeletesObject) {
  {
    sk_std::shared_ptr<TestObj> p(new TestObj(5));
    EXPECT_EQ(TestObj::destroy_count, 0);
  }  // p goes out of scope here
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 8. reset() — becomes null, ref count adjusted
TEST_F(SharedPtrTest, ResetBecomesNull) {
  sk_std::shared_ptr<TestObj> p(new TestObj(3));
  EXPECT_EQ(p.use_count(), 1u);
  p.reset();
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(p.use_count(), 0u);
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 8b. reset() — does not destroy when other references exist
TEST_F(SharedPtrTest, ResetDoesNotDestroyWhenOtherRefs) {
  sk_std::shared_ptr<TestObj> p1(new TestObj(3));
  sk_std::shared_ptr<TestObj> p2(p1);
  p1.reset();
  EXPECT_EQ(TestObj::destroy_count, 0);
  EXPECT_EQ(p2.use_count(), 1u);
}

// 9. reset(T*) — old object released, new object managed
TEST_F(SharedPtrTest, ResetWithNewPointer) {
  sk_std::shared_ptr<TestObj> p(new TestObj(11));
  p.reset(new TestObj(22));
  EXPECT_EQ(TestObj::destroy_count, 1);  // old object destroyed
  EXPECT_EQ(p->value, 22);
  EXPECT_EQ(p.use_count(), 1u);
}

// 10. get() — returns raw pointer
TEST_F(SharedPtrTest, GetReturnsRawPointer) {
  auto* raw = new TestObj(55);
  sk_std::shared_ptr<TestObj> p(raw);
  EXPECT_EQ(p.get(), raw);
}

// 11. operator* and operator-> — dereference works
TEST_F(SharedPtrTest, DereferenceOperators) {
  sk_std::shared_ptr<TestObj> p(new TestObj(77));
  EXPECT_EQ((*p).value, 77);
  EXPECT_EQ(p->value, 77);

  (*p).value = 88;
  EXPECT_EQ(p->value, 88);
}

// 12. operator bool — true when non-null, false when null
TEST_F(SharedPtrTest, BoolConversion) {
  sk_std::shared_ptr<TestObj> null_ptr;
  sk_std::shared_ptr<TestObj> valid_ptr(new TestObj(1));
  EXPECT_FALSE(static_cast<bool>(null_ptr));
  EXPECT_TRUE(static_cast<bool>(valid_ptr));
}

// 13. swap — two shared_ptrs exchange ownership
TEST_F(SharedPtrTest, Swap) {
  auto* raw1 = new TestObj(1);
  auto* raw2 = new TestObj(2);
  sk_std::shared_ptr<TestObj> p1(raw1);
  sk_std::shared_ptr<TestObj> p2(raw2);

  p1.swap(p2);
  EXPECT_EQ(p1.get(), raw2);
  EXPECT_EQ(p2.get(), raw1);
  EXPECT_EQ(p1->value, 2);
  EXPECT_EQ(p2->value, 1);
}

// 13b. Non-member swap
TEST_F(SharedPtrTest, NonMemberSwap) {
  auto* raw1 = new TestObj(10);
  auto* raw2 = new TestObj(20);
  sk_std::shared_ptr<TestObj> p1(raw1);
  sk_std::shared_ptr<TestObj> p2(raw2);

  sk_std::swap(p1, p2);
  EXPECT_EQ(p1.get(), raw2);
  EXPECT_EQ(p2.get(), raw1);
}

// 14. make_shared — creates properly managed object
TEST_F(SharedPtrTest, MakeShared) {
  auto p = sk_std::make_shared<TestObj>(123);
  EXPECT_NE(p.get(), nullptr);
  EXPECT_EQ(p.use_count(), 1u);
  EXPECT_EQ(p->value, 123);
}

// 14b. make_shared with multiple args
TEST_F(SharedPtrTest, MakeSharedMultipleArgs) {
  struct Point {
    int x;
    int y;
    Point(int a, int b) : x(a), y(b) {}
  };
  auto p = sk_std::make_shared<Point>(3, 4);
  EXPECT_EQ(p->x, 3);
  EXPECT_EQ(p->y, 4);
}

// 15. Self-assignment — safe (copy)
TEST_F(SharedPtrTest, SelfAssignmentCopy) {
  sk_std::shared_ptr<TestObj> p(new TestObj(42));
  TestObj* raw = p.get();

  // Use explicit self-assignment (suppress warning with cast)
  auto& ref = p;
  p = ref;

  EXPECT_EQ(p.get(), raw);
  EXPECT_EQ(p.use_count(), 1u);
  EXPECT_EQ(TestObj::destroy_count, 0);
}

// 15b. Self-assignment — safe (move)
TEST_F(SharedPtrTest, SelfAssignmentMove) {
  sk_std::shared_ptr<TestObj> p(new TestObj(42));
  TestObj* raw = p.get();

  p = static_cast<sk_std::shared_ptr<TestObj>&&>(p);

  // After move self-assignment the pointer may become null (valid behavior),
  // but the object must not be double-deleted.
  EXPECT_EQ(TestObj::destroy_count, 0);
  (void)raw;
}

// 16. Multiple copies — correct ref count tracking
TEST_F(SharedPtrTest, MultiplecopiesRefCount) {
  sk_std::shared_ptr<TestObj> p1(new TestObj(0));
  EXPECT_EQ(p1.use_count(), 1u);

  {
    sk_std::shared_ptr<TestObj> p2(p1);
    EXPECT_EQ(p1.use_count(), 2u);
    {
      sk_std::shared_ptr<TestObj> p3(p1);
      EXPECT_EQ(p1.use_count(), 3u);
    }
    // p3 destroyed
    EXPECT_EQ(p1.use_count(), 2u);
    EXPECT_EQ(TestObj::destroy_count, 0);
  }
  // p2 destroyed
  EXPECT_EQ(p1.use_count(), 1u);
  EXPECT_EQ(TestObj::destroy_count, 0);
}

// 17. Null pointer construction — no control block allocated
TEST_F(SharedPtrTest, NullPointerConstruction) {
  sk_std::shared_ptr<TestObj> p(nullptr);
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(p.use_count(), 0u);
  EXPECT_FALSE(static_cast<bool>(p));
}

// 18. reset(nullptr) implicitly via reset()
TEST_F(SharedPtrTest, ResetNullptr) {
  sk_std::shared_ptr<TestObj> p(new TestObj(9));
  p.reset();
  EXPECT_FALSE(static_cast<bool>(p));
  p.reset();  // second reset on null — should be safe
  EXPECT_FALSE(static_cast<bool>(p));
}

}  // namespace
