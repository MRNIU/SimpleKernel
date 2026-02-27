/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "kstd_unique_ptr"

namespace {

struct TestObj {
  int value;
  static int destroy_count;
  explicit TestObj(int v) : value(v) {}
  ~TestObj() { ++destroy_count; }
};
int TestObj::destroy_count = 0;

class UniquePtrTest : public ::testing::Test {
 protected:
  void SetUp() override { TestObj::destroy_count = 0; }
};

// 1. Default construction — null
TEST_F(UniquePtrTest, DefaultConstruction) {
  kstd::unique_ptr<TestObj> p;
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_FALSE(static_cast<bool>(p));
}

// 2. Construction from raw pointer
TEST_F(UniquePtrTest, ConstructionFromRawPointer) {
  kstd::unique_ptr<TestObj> p(new TestObj(42));
  EXPECT_NE(p.get(), nullptr);
  EXPECT_TRUE(static_cast<bool>(p));
  EXPECT_EQ(p->value, 42);
}

// 3. Destructor deletes object
TEST_F(UniquePtrTest, DestructorDeletesObject) {
  {
    kstd::unique_ptr<TestObj> p(new TestObj(5));
    EXPECT_EQ(TestObj::destroy_count, 0);
  }
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 4. Move construction — source becomes null
TEST_F(UniquePtrTest, MoveConstruction) {
  kstd::unique_ptr<TestObj> p1(new TestObj(99));
  TestObj* raw = p1.get();

  kstd::unique_ptr<TestObj> p2(static_cast<kstd::unique_ptr<TestObj>&&>(p1));
  EXPECT_EQ(p1.get(), nullptr);
  EXPECT_EQ(p2.get(), raw);
  EXPECT_EQ(TestObj::destroy_count, 0);
}

// 5. Move assignment
TEST_F(UniquePtrTest, MoveAssignment) {
  kstd::unique_ptr<TestObj> p1(new TestObj(7));
  kstd::unique_ptr<TestObj> p2(new TestObj(8));
  TestObj* raw1 = p1.get();

  p2 = static_cast<kstd::unique_ptr<TestObj>&&>(p1);
  EXPECT_EQ(TestObj::destroy_count, 1);  // p2's old object destroyed
  EXPECT_EQ(p1.get(), nullptr);
  EXPECT_EQ(p2.get(), raw1);
}

// 6. release() — returns pointer, unique_ptr becomes null
TEST_F(UniquePtrTest, Release) {
  auto* raw = new TestObj(10);
  kstd::unique_ptr<TestObj> p(raw);
  TestObj* released = p.release();
  EXPECT_EQ(released, raw);
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(TestObj::destroy_count, 0);  // not deleted!
  delete released;                       // manual cleanup
}

// 7. reset() — becomes null, deletes old
TEST_F(UniquePtrTest, ResetBecomesNull) {
  kstd::unique_ptr<TestObj> p(new TestObj(3));
  p.reset();
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 8. reset(T*) — replaces managed object
TEST_F(UniquePtrTest, ResetWithNewPointer) {
  kstd::unique_ptr<TestObj> p(new TestObj(11));
  p.reset(new TestObj(22));
  EXPECT_EQ(TestObj::destroy_count, 1);
  EXPECT_EQ(p->value, 22);
}

// 9. swap
TEST_F(UniquePtrTest, Swap) {
  auto* raw1 = new TestObj(1);
  auto* raw2 = new TestObj(2);
  kstd::unique_ptr<TestObj> p1(raw1);
  kstd::unique_ptr<TestObj> p2(raw2);

  p1.swap(p2);
  EXPECT_EQ(p1.get(), raw2);
  EXPECT_EQ(p2.get(), raw1);
}

// 10. Non-member swap
TEST_F(UniquePtrTest, NonMemberSwap) {
  auto* raw1 = new TestObj(10);
  auto* raw2 = new TestObj(20);
  kstd::unique_ptr<TestObj> p1(raw1);
  kstd::unique_ptr<TestObj> p2(raw2);

  kstd::swap(p1, p2);
  EXPECT_EQ(p1.get(), raw2);
  EXPECT_EQ(p2.get(), raw1);
}

// 11. Dereference operators
TEST_F(UniquePtrTest, DereferenceOperators) {
  kstd::unique_ptr<TestObj> p(new TestObj(77));
  EXPECT_EQ((*p).value, 77);
  EXPECT_EQ(p->value, 77);
  (*p).value = 88;
  EXPECT_EQ(p->value, 88);
}

// 12. Bool conversion
TEST_F(UniquePtrTest, BoolConversion) {
  kstd::unique_ptr<TestObj> null_ptr;
  kstd::unique_ptr<TestObj> valid_ptr(new TestObj(1));
  EXPECT_FALSE(static_cast<bool>(null_ptr));
  EXPECT_TRUE(static_cast<bool>(valid_ptr));
}

// 13. nullptr assignment
TEST_F(UniquePtrTest, NullptrAssignment) {
  kstd::unique_ptr<TestObj> p(new TestObj(5));
  p = nullptr;
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 14. nullptr construction
TEST_F(UniquePtrTest, NullptrConstruction) {
  kstd::unique_ptr<TestObj> p(nullptr);
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_FALSE(static_cast<bool>(p));
}

// 15. Custom deleter
TEST_F(UniquePtrTest, CustomDeleter) {
  static int custom_delete_count = 0;
  custom_delete_count = 0;

  struct CustomDeleter {
    auto operator()(TestObj* ptr) const noexcept -> void {
      ++custom_delete_count;
      delete ptr;
    }
  };

  {
    kstd::unique_ptr<TestObj, CustomDeleter> p(new TestObj(1));
    EXPECT_EQ(custom_delete_count, 0);
  }
  EXPECT_EQ(custom_delete_count, 1);
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 16. get_deleter
TEST_F(UniquePtrTest, GetDeleter) {
  kstd::unique_ptr<TestObj> p(new TestObj(1));
  auto& d = p.get_deleter();
  // Just verify we can call it — default_delete should work
  (void)d;
}

// 17. make_unique
TEST_F(UniquePtrTest, MakeUnique) {
  auto p = kstd::make_unique<TestObj>(123);
  EXPECT_NE(p.get(), nullptr);
  EXPECT_EQ(p->value, 123);
}

// 18. make_unique with multiple args
TEST_F(UniquePtrTest, MakeUniqueMultipleArgs) {
  struct Point {
    int x;
    int y;
    Point(int a, int b) : x(a), y(b) {}
  };
  auto p = kstd::make_unique<Point>(3, 4);
  EXPECT_EQ(p->x, 3);
  EXPECT_EQ(p->y, 4);
}

// 19. Polymorphic — base unique_ptr managing derived
TEST_F(UniquePtrTest, Polymorphic) {
  struct Base {
    virtual ~Base() = default;
    virtual auto GetValue() -> int { return 0; }
  };
  struct Derived : Base {
    int val;
    explicit Derived(int v) : val(v) {}
    auto GetValue() -> int override { return val; }
  };

  kstd::unique_ptr<Base> p(new Derived(42));
  EXPECT_EQ(p->GetValue(), 42);
}

// 20. Self move assignment — safe
TEST_F(UniquePtrTest, SelfMoveAssignment) {
  kstd::unique_ptr<TestObj> p(new TestObj(42));
  p = static_cast<kstd::unique_ptr<TestObj>&&>(p);
  // Object must not be double-deleted
  EXPECT_EQ(TestObj::destroy_count, 0);
}

// 21. Comparison operators
TEST_F(UniquePtrTest, ComparisonOperators) {
  kstd::unique_ptr<TestObj> null_ptr;
  kstd::unique_ptr<TestObj> p1(new TestObj(1));
  kstd::unique_ptr<TestObj> p2(new TestObj(2));

  EXPECT_TRUE(null_ptr == nullptr);
  EXPECT_TRUE(nullptr == null_ptr);
  EXPECT_FALSE(p1 == nullptr);
  EXPECT_FALSE(p1 == p2);
  EXPECT_TRUE(p1 != p2);
  EXPECT_TRUE(p1 != nullptr);
}

// ── Array specialization tests ──────────────────────────────────────────────

struct ArrayObj {
  int value;
  static int destroy_count;
  ArrayObj() : value(0) {}
  explicit ArrayObj(int v) : value(v) {}
  ~ArrayObj() { ++destroy_count; }
};
int ArrayObj::destroy_count = 0;

class UniquePtrArrayTest : public ::testing::Test {
 protected:
  void SetUp() override { ArrayObj::destroy_count = 0; }
};

// 22. Array default construction
TEST_F(UniquePtrArrayTest, DefaultConstruction) {
  kstd::unique_ptr<ArrayObj[]> p;
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_FALSE(static_cast<bool>(p));
}

// 23. Array construction + destructor calls delete[]
TEST_F(UniquePtrArrayTest, ConstructionAndDestruction) {
  {
    kstd::unique_ptr<ArrayObj[]> p(new ArrayObj[3]);
    EXPECT_NE(p.get(), nullptr);
    EXPECT_EQ(ArrayObj::destroy_count, 0);
  }
  EXPECT_EQ(ArrayObj::destroy_count, 3);
}

// 24. Array subscript operator
TEST_F(UniquePtrArrayTest, SubscriptOperator) {
  kstd::unique_ptr<ArrayObj[]> p(new ArrayObj[3]);
  p[0].value = 10;
  p[1].value = 20;
  p[2].value = 30;
  EXPECT_EQ(p[0].value, 10);
  EXPECT_EQ(p[1].value, 20);
  EXPECT_EQ(p[2].value, 30);
}

// 25. Array move construction
TEST_F(UniquePtrArrayTest, MoveConstruction) {
  kstd::unique_ptr<ArrayObj[]> p1(new ArrayObj[2]);
  ArrayObj* raw = p1.get();
  kstd::unique_ptr<ArrayObj[]> p2(
      static_cast<kstd::unique_ptr<ArrayObj[]>&&>(p1));
  EXPECT_EQ(p1.get(), nullptr);
  EXPECT_EQ(p2.get(), raw);
}

// 26. Array release
TEST_F(UniquePtrArrayTest, Release) {
  auto* raw = new ArrayObj[2];
  kstd::unique_ptr<ArrayObj[]> p(raw);
  ArrayObj* released = p.release();
  EXPECT_EQ(released, raw);
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(ArrayObj::destroy_count, 0);
  delete[] released;
}

// 27. Array reset
TEST_F(UniquePtrArrayTest, Reset) {
  kstd::unique_ptr<ArrayObj[]> p(new ArrayObj[2]);
  p.reset();
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(ArrayObj::destroy_count, 2);
}

// 28. Array swap
TEST_F(UniquePtrArrayTest, Swap) {
  auto* raw1 = new ArrayObj[1];
  auto* raw2 = new ArrayObj[1];
  kstd::unique_ptr<ArrayObj[]> p1(raw1);
  kstd::unique_ptr<ArrayObj[]> p2(raw2);
  p1.swap(p2);
  EXPECT_EQ(p1.get(), raw2);
  EXPECT_EQ(p2.get(), raw1);
}

// 29. Array nullptr assignment
TEST_F(UniquePtrArrayTest, NullptrAssignment) {
  kstd::unique_ptr<ArrayObj[]> p(new ArrayObj[2]);
  p = nullptr;
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(ArrayObj::destroy_count, 2);
}

}  // namespace
