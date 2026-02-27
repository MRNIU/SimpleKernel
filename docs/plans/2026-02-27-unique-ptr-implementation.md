# sk_std::unique_ptr Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement `sk_std::unique_ptr` (primary + array specialization) and replace all raw `new`/`delete` ownership patterns across the project.

**Architecture:** Move-only owning smart pointer with custom deleter support. Primary template for single objects, partial specialization for arrays. No control block (unlike shared_ptr) — just pointer + deleter stored inline.

**Tech Stack:** C++23, freestanding, GoogleTest for unit tests, no exceptions/RTTI.

---

### Task 1: Create sk_unique_ptr header

**Files:**
- Create: `src/libcxx/include/sk_unique_ptr`

**Step 1: Write the unique_ptr implementation**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_UNIQUE_PTR_
#define SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_UNIQUE_PTR_

// @note sk_std::unique_ptr requires heap allocation via ::operator new/delete.
// It MUST only be used after MemoryInit() has been called during kernel boot.
// Using unique_ptr before MemoryInit() will result in undefined behavior or
// a kernel crash.

#include <cstddef>

namespace sk_std {

// ── Default deleters ─────────────────────────────────────────────────────────

/// @brief Default deleter for single objects (calls `delete`).
template <typename T>
struct default_delete {
  constexpr default_delete() noexcept = default;
  auto operator()(T* ptr) const noexcept -> void { delete ptr; }
};

/// @brief Default deleter for arrays (calls `delete[]`).
template <typename T>
struct default_delete<T[]> {
  constexpr default_delete() noexcept = default;
  auto operator()(T* ptr) const noexcept -> void { delete[] ptr; }
};

// ── Primary template: unique_ptr<T, Deleter> ────────────────────────────────

/// @brief Move-only owning smart pointer for single objects.
///
/// Holds exclusive ownership of a dynamically allocated object and deletes it
/// when the unique_ptr is destroyed or reset.
///
/// @note Thread safety: unique_ptr itself is NOT thread-safe. Callers must
///       synchronize concurrent access to the same unique_ptr instance.
///
/// @pre  MemoryInit() must have been called before constructing any unique_ptr.
/// @tparam T      Type of the managed object.
/// @tparam Deleter Callable type used to destroy the managed object.
template <typename T, typename Deleter = default_delete<T>>
class unique_ptr {
 public:
  // ── Constructors ──────────────────────────────────────────────────────────

  /// @brief Default constructor — null state.
  constexpr unique_ptr() noexcept : ptr_(nullptr), deleter_() {}

  /// @brief Construct from nullptr literal.
  constexpr unique_ptr(decltype(nullptr)) noexcept  // NOLINT
      : ptr_(nullptr), deleter_() {}

  /// @brief Construct from raw pointer, taking ownership.
  ///
  /// @pre  ptr was allocated with ::operator new (or nullptr).
  /// @post get() == ptr
  explicit unique_ptr(T* ptr) noexcept : ptr_(ptr), deleter_() {}

  /// @brief Construct from raw pointer with custom deleter.
  unique_ptr(T* ptr, const Deleter& d) noexcept : ptr_(ptr), deleter_(d) {}

  /// @brief Move constructor — transfers ownership, source becomes null.
  unique_ptr(unique_ptr&& other) noexcept
      : ptr_(other.ptr_), deleter_(static_cast<Deleter&&>(other.deleter_)) {
    other.ptr_ = nullptr;
  }

  /// @brief No copy construction.
  unique_ptr(const unique_ptr&) = delete;

  /// @brief Destructor — deletes managed object if non-null.
  ~unique_ptr() {
    if (ptr_ != nullptr) {
      deleter_(ptr_);
    }
  }

  // ── Assignment ────────────────────────────────────────────────────────────

  /// @brief Move assignment — releases current, transfers other's ownership.
  auto operator=(unique_ptr&& other) noexcept -> unique_ptr& {
    if (this != &other) {
      reset();
      ptr_ = other.ptr_;
      deleter_ = static_cast<Deleter&&>(other.deleter_);
      other.ptr_ = nullptr;
    }
    return *this;
  }

  /// @brief No copy assignment.
  auto operator=(const unique_ptr&) -> unique_ptr& = delete;

  /// @brief Assign nullptr — equivalent to reset().
  auto operator=(decltype(nullptr)) noexcept -> unique_ptr& {
    reset();
    return *this;
  }

  // ── Modifiers ─────────────────────────────────────────────────────────────

  /// @brief Release ownership and return the raw pointer.
  ///
  /// @post get() == nullptr. Caller is responsible for deleting the object.
  [[nodiscard]] auto release() noexcept -> T* {
    T* tmp = ptr_;
    ptr_ = nullptr;
    return tmp;
  }

  /// @brief Replace managed object (delete old, take ownership of new).
  auto reset(T* ptr = nullptr) noexcept -> void {
    T* old = ptr_;
    ptr_ = ptr;
    if (old != nullptr) {
      deleter_(old);
    }
  }

  /// @brief Swap two unique_ptrs.
  auto swap(unique_ptr& other) noexcept -> void {
    T* tmp_ptr = ptr_;
    ptr_ = other.ptr_;
    other.ptr_ = tmp_ptr;

    Deleter tmp_del = static_cast<Deleter&&>(deleter_);
    deleter_ = static_cast<Deleter&&>(other.deleter_);
    other.deleter_ = static_cast<Deleter&&>(tmp_del);
  }

  // ── Observers ─────────────────────────────────────────────────────────────

  /// @brief Return the stored raw pointer (nullptr if empty).
  [[nodiscard]] auto get() const noexcept -> T* { return ptr_; }

  /// @brief Return reference to the deleter.
  [[nodiscard]] auto get_deleter() noexcept -> Deleter& { return deleter_; }

  /// @brief Return const reference to the deleter.
  [[nodiscard]] auto get_deleter() const noexcept -> const Deleter& {
    return deleter_;
  }

  /// @brief Return true if this unique_ptr owns a non-null object.
  [[nodiscard]] explicit operator bool() const noexcept {
    return ptr_ != nullptr;
  }

  /// @brief Dereference the managed object.
  /// @pre  get() != nullptr
  [[nodiscard]] auto operator*() const noexcept -> T& { return *ptr_; }

  /// @brief Member access on the managed object.
  /// @pre  get() != nullptr
  [[nodiscard]] auto operator->() const noexcept -> T* { return ptr_; }

 private:
  T* ptr_;
  Deleter deleter_;
};

// ── Array specialization: unique_ptr<T[], Deleter> ──────────────────────────

/// @brief Move-only owning smart pointer for dynamically allocated arrays.
///
/// Same as the primary template but provides operator[] instead of
/// operator* and operator->. Uses delete[] by default.
///
/// @pre  MemoryInit() must have been called before constructing any unique_ptr.
/// @tparam T      Element type of the managed array.
/// @tparam Deleter Callable type used to destroy the managed array.
template <typename T, typename Deleter>
class unique_ptr<T[], Deleter> {
 public:
  // ── Constructors ──────────────────────────────────────────────────────────

  constexpr unique_ptr() noexcept : ptr_(nullptr), deleter_() {}

  constexpr unique_ptr(decltype(nullptr)) noexcept  // NOLINT
      : ptr_(nullptr), deleter_() {}

  explicit unique_ptr(T* ptr) noexcept : ptr_(ptr), deleter_() {}

  unique_ptr(T* ptr, const Deleter& d) noexcept : ptr_(ptr), deleter_(d) {}

  unique_ptr(unique_ptr&& other) noexcept
      : ptr_(other.ptr_), deleter_(static_cast<Deleter&&>(other.deleter_)) {
    other.ptr_ = nullptr;
  }

  unique_ptr(const unique_ptr&) = delete;

  ~unique_ptr() {
    if (ptr_ != nullptr) {
      deleter_(ptr_);
    }
  }

  // ── Assignment ────────────────────────────────────────────────────────────

  auto operator=(unique_ptr&& other) noexcept -> unique_ptr& {
    if (this != &other) {
      reset();
      ptr_ = other.ptr_;
      deleter_ = static_cast<Deleter&&>(other.deleter_);
      other.ptr_ = nullptr;
    }
    return *this;
  }

  auto operator=(const unique_ptr&) -> unique_ptr& = delete;

  auto operator=(decltype(nullptr)) noexcept -> unique_ptr& {
    reset();
    return *this;
  }

  // ── Modifiers ─────────────────────────────────────────────────────────────

  [[nodiscard]] auto release() noexcept -> T* {
    T* tmp = ptr_;
    ptr_ = nullptr;
    return tmp;
  }

  auto reset(T* ptr = nullptr) noexcept -> void {
    T* old = ptr_;
    ptr_ = ptr;
    if (old != nullptr) {
      deleter_(old);
    }
  }

  auto swap(unique_ptr& other) noexcept -> void {
    T* tmp_ptr = ptr_;
    ptr_ = other.ptr_;
    other.ptr_ = tmp_ptr;

    Deleter tmp_del = static_cast<Deleter&&>(deleter_);
    deleter_ = static_cast<Deleter&&>(other.deleter_);
    other.deleter_ = static_cast<Deleter&&>(tmp_del);
  }

  // ── Observers ─────────────────────────────────────────────────────────────

  [[nodiscard]] auto get() const noexcept -> T* { return ptr_; }

  [[nodiscard]] auto get_deleter() noexcept -> Deleter& { return deleter_; }

  [[nodiscard]] auto get_deleter() const noexcept -> const Deleter& {
    return deleter_;
  }

  [[nodiscard]] explicit operator bool() const noexcept {
    return ptr_ != nullptr;
  }

  /// @brief Array subscript access.
  /// @pre  get() != nullptr && idx is within bounds
  [[nodiscard]] auto operator[](size_t idx) const -> T& { return ptr_[idx]; }

 private:
  T* ptr_;
  Deleter deleter_;
};

// ── Non-member functions ────────────────────────────────────────────────────

/// @brief Swap two unique_ptrs (non-member overload).
template <typename T, typename D>
auto swap(unique_ptr<T, D>& a, unique_ptr<T, D>& b) noexcept -> void {
  a.swap(b);
}

/// @brief Equality comparison.
template <typename T1, typename D1, typename T2, typename D2>
auto operator==(const unique_ptr<T1, D1>& a, const unique_ptr<T2, D2>& b)
    -> bool {
  return a.get() == b.get();
}

/// @brief Compare with nullptr.
template <typename T, typename D>
auto operator==(const unique_ptr<T, D>& p, decltype(nullptr)) -> bool {
  return p.get() == nullptr;
}

/// @brief Compare nullptr with unique_ptr.
template <typename T, typename D>
auto operator==(decltype(nullptr), const unique_ptr<T, D>& p) -> bool {
  return p.get() == nullptr;
}

/// @brief Inequality comparison.
template <typename T1, typename D1, typename T2, typename D2>
auto operator!=(const unique_ptr<T1, D1>& a, const unique_ptr<T2, D2>& b)
    -> bool {
  return !(a == b);
}

/// @brief Inequality with nullptr.
template <typename T, typename D>
auto operator!=(const unique_ptr<T, D>& p, decltype(nullptr)) -> bool {
  return !(p == nullptr);
}

/// @brief Inequality nullptr with unique_ptr.
template <typename T, typename D>
auto operator!=(decltype(nullptr), const unique_ptr<T, D>& p) -> bool {
  return !(p == nullptr);
}

/// @brief Create a unique_ptr, constructing T in-place from args.
///
/// Equivalent to unique_ptr<T>(new T(args...)).
///
/// @pre  MemoryInit() must have been called.
template <typename T, typename... Args>
auto make_unique(Args&&... args) -> unique_ptr<T> {
  return unique_ptr<T>(new T(static_cast<Args&&>(args)...));
}

}  // namespace sk_std

#endif /* SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_UNIQUE_PTR_ */
```

**Step 2: Verify the file compiles with no issues**

Run: `lsp_diagnostics` on the new file.

---

### Task 2: Write comprehensive unit tests

**Files:**
- Create: `tests/unit_test/sk_unique_ptr_test.cpp`
- Modify: `tests/unit_test/CMakeLists.txt` — add `sk_unique_ptr_test.cpp` to source list

**Step 1: Write test file**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_unique_ptr"

#include <gtest/gtest.h>

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
  sk_std::unique_ptr<TestObj> p;
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_FALSE(static_cast<bool>(p));
}

// 2. Construction from raw pointer
TEST_F(UniquePtrTest, ConstructionFromRawPointer) {
  sk_std::unique_ptr<TestObj> p(new TestObj(42));
  EXPECT_NE(p.get(), nullptr);
  EXPECT_TRUE(static_cast<bool>(p));
  EXPECT_EQ(p->value, 42);
}

// 3. Destructor deletes object
TEST_F(UniquePtrTest, DestructorDeletesObject) {
  {
    sk_std::unique_ptr<TestObj> p(new TestObj(5));
    EXPECT_EQ(TestObj::destroy_count, 0);
  }
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 4. Move construction — source becomes null
TEST_F(UniquePtrTest, MoveConstruction) {
  sk_std::unique_ptr<TestObj> p1(new TestObj(99));
  TestObj* raw = p1.get();

  sk_std::unique_ptr<TestObj> p2(
      static_cast<sk_std::unique_ptr<TestObj>&&>(p1));
  EXPECT_EQ(p1.get(), nullptr);
  EXPECT_EQ(p2.get(), raw);
  EXPECT_EQ(TestObj::destroy_count, 0);
}

// 5. Move assignment
TEST_F(UniquePtrTest, MoveAssignment) {
  sk_std::unique_ptr<TestObj> p1(new TestObj(7));
  sk_std::unique_ptr<TestObj> p2(new TestObj(8));
  TestObj* raw1 = p1.get();

  p2 = static_cast<sk_std::unique_ptr<TestObj>&&>(p1);
  EXPECT_EQ(TestObj::destroy_count, 1);  // p2's old object destroyed
  EXPECT_EQ(p1.get(), nullptr);
  EXPECT_EQ(p2.get(), raw1);
}

// 6. release() — returns pointer, unique_ptr becomes null
TEST_F(UniquePtrTest, Release) {
  auto* raw = new TestObj(10);
  sk_std::unique_ptr<TestObj> p(raw);
  TestObj* released = p.release();
  EXPECT_EQ(released, raw);
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(TestObj::destroy_count, 0);  // not deleted!
  delete released;  // manual cleanup
}

// 7. reset() — becomes null, deletes old
TEST_F(UniquePtrTest, ResetBecomesNull) {
  sk_std::unique_ptr<TestObj> p(new TestObj(3));
  p.reset();
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 8. reset(T*) — replaces managed object
TEST_F(UniquePtrTest, ResetWithNewPointer) {
  sk_std::unique_ptr<TestObj> p(new TestObj(11));
  p.reset(new TestObj(22));
  EXPECT_EQ(TestObj::destroy_count, 1);
  EXPECT_EQ(p->value, 22);
}

// 9. swap
TEST_F(UniquePtrTest, Swap) {
  auto* raw1 = new TestObj(1);
  auto* raw2 = new TestObj(2);
  sk_std::unique_ptr<TestObj> p1(raw1);
  sk_std::unique_ptr<TestObj> p2(raw2);

  p1.swap(p2);
  EXPECT_EQ(p1.get(), raw2);
  EXPECT_EQ(p2.get(), raw1);
}

// 10. Non-member swap
TEST_F(UniquePtrTest, NonMemberSwap) {
  auto* raw1 = new TestObj(10);
  auto* raw2 = new TestObj(20);
  sk_std::unique_ptr<TestObj> p1(raw1);
  sk_std::unique_ptr<TestObj> p2(raw2);

  sk_std::swap(p1, p2);
  EXPECT_EQ(p1.get(), raw2);
  EXPECT_EQ(p2.get(), raw1);
}

// 11. Dereference operators
TEST_F(UniquePtrTest, DereferenceOperators) {
  sk_std::unique_ptr<TestObj> p(new TestObj(77));
  EXPECT_EQ((*p).value, 77);
  EXPECT_EQ(p->value, 77);
  (*p).value = 88;
  EXPECT_EQ(p->value, 88);
}

// 12. Bool conversion
TEST_F(UniquePtrTest, BoolConversion) {
  sk_std::unique_ptr<TestObj> null_ptr;
  sk_std::unique_ptr<TestObj> valid_ptr(new TestObj(1));
  EXPECT_FALSE(static_cast<bool>(null_ptr));
  EXPECT_TRUE(static_cast<bool>(valid_ptr));
}

// 13. nullptr assignment
TEST_F(UniquePtrTest, NullptrAssignment) {
  sk_std::unique_ptr<TestObj> p(new TestObj(5));
  p = nullptr;
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 14. nullptr construction
TEST_F(UniquePtrTest, NullptrConstruction) {
  sk_std::unique_ptr<TestObj> p(nullptr);
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
    sk_std::unique_ptr<TestObj, CustomDeleter> p(new TestObj(1));
    EXPECT_EQ(custom_delete_count, 0);
  }
  EXPECT_EQ(custom_delete_count, 1);
  EXPECT_EQ(TestObj::destroy_count, 1);
}

// 16. get_deleter
TEST_F(UniquePtrTest, GetDeleter) {
  sk_std::unique_ptr<TestObj> p(new TestObj(1));
  auto& d = p.get_deleter();
  // Just verify we can call it — default_delete should work
  (void)d;
}

// 17. make_unique
TEST_F(UniquePtrTest, MakeUnique) {
  auto p = sk_std::make_unique<TestObj>(123);
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
  auto p = sk_std::make_unique<Point>(3, 4);
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

  sk_std::unique_ptr<Base> p(new Derived(42));
  EXPECT_EQ(p->GetValue(), 42);
}

// 20. Self move assignment — safe
TEST_F(UniquePtrTest, SelfMoveAssignment) {
  sk_std::unique_ptr<TestObj> p(new TestObj(42));
  p = static_cast<sk_std::unique_ptr<TestObj>&&>(p);
  // Object must not be double-deleted
  EXPECT_EQ(TestObj::destroy_count, 0);
}

// 21. Comparison operators
TEST_F(UniquePtrTest, ComparisonOperators) {
  sk_std::unique_ptr<TestObj> null_ptr;
  sk_std::unique_ptr<TestObj> p1(new TestObj(1));
  sk_std::unique_ptr<TestObj> p2(new TestObj(2));

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
  sk_std::unique_ptr<ArrayObj[]> p;
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_FALSE(static_cast<bool>(p));
}

// 23. Array construction + destructor calls delete[]
TEST_F(UniquePtrArrayTest, ConstructionAndDestruction) {
  {
    sk_std::unique_ptr<ArrayObj[]> p(new ArrayObj[3]);
    EXPECT_NE(p.get(), nullptr);
    EXPECT_EQ(ArrayObj::destroy_count, 0);
  }
  EXPECT_EQ(ArrayObj::destroy_count, 3);
}

// 24. Array subscript operator
TEST_F(UniquePtrArrayTest, SubscriptOperator) {
  sk_std::unique_ptr<ArrayObj[]> p(new ArrayObj[3]);
  p[0].value = 10;
  p[1].value = 20;
  p[2].value = 30;
  EXPECT_EQ(p[0].value, 10);
  EXPECT_EQ(p[1].value, 20);
  EXPECT_EQ(p[2].value, 30);
}

// 25. Array move construction
TEST_F(UniquePtrArrayTest, MoveConstruction) {
  sk_std::unique_ptr<ArrayObj[]> p1(new ArrayObj[2]);
  ArrayObj* raw = p1.get();
  sk_std::unique_ptr<ArrayObj[]> p2(
      static_cast<sk_std::unique_ptr<ArrayObj[]>&&>(p1));
  EXPECT_EQ(p1.get(), nullptr);
  EXPECT_EQ(p2.get(), raw);
}

// 26. Array release
TEST_F(UniquePtrArrayTest, Release) {
  auto* raw = new ArrayObj[2];
  sk_std::unique_ptr<ArrayObj[]> p(raw);
  ArrayObj* released = p.release();
  EXPECT_EQ(released, raw);
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(ArrayObj::destroy_count, 0);
  delete[] released;
}

// 27. Array reset
TEST_F(UniquePtrArrayTest, Reset) {
  sk_std::unique_ptr<ArrayObj[]> p(new ArrayObj[2]);
  p.reset();
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(ArrayObj::destroy_count, 2);
}

// 28. Array swap
TEST_F(UniquePtrArrayTest, Swap) {
  auto* raw1 = new ArrayObj[1];
  auto* raw2 = new ArrayObj[1];
  sk_std::unique_ptr<ArrayObj[]> p1(raw1);
  sk_std::unique_ptr<ArrayObj[]> p2(raw2);
  p1.swap(p2);
  EXPECT_EQ(p1.get(), raw2);
  EXPECT_EQ(p2.get(), raw1);
}

// 29. Array nullptr assignment
TEST_F(UniquePtrArrayTest, NullptrAssignment) {
  sk_std::unique_ptr<ArrayObj[]> p(new ArrayObj[2]);
  p = nullptr;
  EXPECT_EQ(p.get(), nullptr);
  EXPECT_EQ(ArrayObj::destroy_count, 2);
}

}  // namespace
```

**Step 2: Add test file to CMakeLists.txt**

In `tests/unit_test/CMakeLists.txt`, add `sk_unique_ptr_test.cpp` after `sk_shared_ptr_test.cpp` (line 26).

**Step 3: Build and run tests**

Run: `cmake --preset build_x86_64 && cd build_x86_64 && make unit-test`
Expected: All unique_ptr tests PASS

**Step 4: Commit**

```bash
git add src/libcxx/include/sk_unique_ptr tests/unit_test/sk_unique_ptr_test.cpp tests/unit_test/CMakeLists.txt
git commit --signoff -m "feat(libcxx): add sk_std::unique_ptr with array specialization

Implement move-only owning smart pointer with custom deleter support.
Primary template for single objects, partial specialization for arrays.
Includes make_unique factory, comparison operators, and non-member swap."
```

---

### Task 3: Batch 1 — Replace schedulers in CpuSchedData

**Files:**
- Modify: `src/task/include/task_manager.hpp:34` — change `SchedulerBase*` to `sk_std::unique_ptr<SchedulerBase>`
- Modify: `src/task/task_manager.cpp:45-47` — use `make_unique` for scheduler creation
- Modify: `src/task/task_manager.cpp:61` — use `.get()` for `Enqueue()`
- Modify: `src/task/task_manager.cpp:110-111` — use `.get()` for scheduler access
- Modify: `src/task/task_manager.cpp:218-225` — remove manual `delete scheduler` in destructor

**Step 1: Add `#include "sk_unique_ptr"` to task_manager.hpp**

Add after the existing includes.

**Step 2: Change CpuSchedData::schedulers type**

Change line 34 from:
```cpp
std::array<SchedulerBase*, SchedPolicy::kPolicyCount> schedulers{};
```
to:
```cpp
std::array<sk_std::unique_ptr<SchedulerBase>, SchedPolicy::kPolicyCount> schedulers{};
```

**Step 3: Update InitCurrentCore() — use make_unique**

In `task_manager.cpp`, change lines 45-47 from:
```cpp
cpu_sched.schedulers[SchedPolicy::kRealTime] = new FifoScheduler();
cpu_sched.schedulers[SchedPolicy::kNormal] = new RoundRobinScheduler();
cpu_sched.schedulers[SchedPolicy::kIdle] = new IdleScheduler();
```
to:
```cpp
cpu_sched.schedulers[SchedPolicy::kRealTime] = sk_std::make_unique<FifoScheduler>();
cpu_sched.schedulers[SchedPolicy::kNormal] = sk_std::make_unique<RoundRobinScheduler>();
cpu_sched.schedulers[SchedPolicy::kIdle] = sk_std::make_unique<IdleScheduler>();
```

Note: This requires that `unique_ptr<Derived>` be implicitly convertible to `unique_ptr<Base>`, which our implementation does NOT support (no converting move constructor). Instead, use explicit construction:
```cpp
cpu_sched.schedulers[SchedPolicy::kRealTime] = sk_std::unique_ptr<SchedulerBase>(new FifoScheduler());
cpu_sched.schedulers[SchedPolicy::kNormal] = sk_std::unique_ptr<SchedulerBase>(new RoundRobinScheduler());
cpu_sched.schedulers[SchedPolicy::kIdle] = sk_std::unique_ptr<SchedulerBase>(new IdleScheduler());
```

**Step 4: Update all `schedulers[X]` accesses to use `.get()` where raw pointer is needed**

- Line 44: `if (!cpu_sched.schedulers[SchedPolicy::kNormal])` — this works via `operator bool()`
- Line 60: `if (cpu_sched.schedulers[SchedPolicy::kIdle])` — works via `operator bool()`
- Line 61: `cpu_sched.schedulers[SchedPolicy::kIdle]->Enqueue(idle_task)` — works via `operator->()`
- Line 110: `if (cpu_sched.schedulers[task->policy])` — works via `operator bool()`
- Line 111: `cpu_sched.schedulers[task->policy]->Enqueue(task)` — works via `operator->()`

These all work without changes because unique_ptr provides `operator bool()` and `operator->()`.

**Step 5: Remove manual delete in ~TaskManager()**

Change lines 218-225 from:
```cpp
TaskManager::~TaskManager() {
  for (auto& cpu_sched : cpu_schedulers_) {
    for (auto& scheduler : cpu_sched.schedulers) {
      delete scheduler;
      scheduler = nullptr;
    }
  }
}
```
to:
```cpp
TaskManager::~TaskManager() {
  // unique_ptr in cpu_schedulers_.schedulers[] auto-deletes on destruction
}
```

Or simply: `TaskManager::~TaskManager() = default;` — but since the destructor was non-default before, keep the body with just a comment for clarity.

**Step 6: Check all other scheduler access sites**

Search for `schedulers[` across the codebase and update any that use raw pointer operations.
Files to check: `schedule.cpp`, `tick_update.cpp`, `sleep.cpp`, `block.cpp`, `wakeup.cpp`.

All these files access schedulers via `->` (e.g., `schedulers[policy]->Pick()`) which works directly with unique_ptr.

**Step 7: Build and run tests**

Run: `cmake --preset build_x86_64 && cd build_x86_64 && make unit-test`
Expected: All tests PASS

**Step 8: Commit**

```bash
git add src/task/include/task_manager.hpp src/task/task_manager.cpp
git commit --signoff -m "refactor(task): use unique_ptr for scheduler ownership in CpuSchedData

Replace raw SchedulerBase* array with unique_ptr to ensure automatic
cleanup and express ownership semantics explicitly."
```

---

### Task 4: Batch 2 — Replace IoBuffer in device.cpp

**Files:**
- Modify: `src/device/include/device_node.hpp:106` — change `IoBuffer* dma_buffer` to `sk_std::unique_ptr<IoBuffer> dma_buffer` (NOTE: device_node.hpp is header-only device framework — check if it has a copy constructor that copies dma_buffer)
- Modify: `src/device/device.cpp:55-64` — use make_unique, remove manual delete

**Important**: `device_node.hpp` has a copy constructor and copy assignment that copy `dma_buffer` as a raw pointer. With unique_ptr, copy would be deleted. We need to check if DeviceNode is ever copied. If so, we may need to keep raw pointer here or add move semantics.

**Step 1: Check if DeviceNode is copied anywhere**

Search for `DeviceNode` copy/assignment usage. If DeviceNode is stored in arrays or vectors, it may be moved/copied.

If copies are found, convert DeviceNode's copy constructor to a move constructor, or keep `dma_buffer` as raw pointer and only manage it via unique_ptr locally in device.cpp.

**Conservative approach**: Use `unique_ptr` locally in device.cpp for construction, then `.release()` into the raw `dma_buffer` field. This avoids changing the device_node.hpp header (which is header-only and shared).

In `device.cpp` lines 55-64, change from:
```cpp
auto* buf = new IoBuffer(VirtioBlkDriver<PlatformTraits>::kMinDmaBufferSize);
if (buf != nullptr && buf->IsValid()) {
  devs[i]->dma_buffer = buf;
  ...
} else {
  ...
  delete buf;
}
```
to:
```cpp
auto buf = sk_std::make_unique<IoBuffer>(VirtioBlkDriver<PlatformTraits>::kMinDmaBufferSize);
if (buf && buf->IsValid()) {
  devs[i]->dma_buffer = buf.release();
  ...
} else {
  ...
  // unique_ptr auto-deletes on scope exit
}
```

This is a minimal, safe change. The `dma_buffer` field stays raw in device_node.hpp because DeviceNode is a framework header we should not modify per AGENTS.md constraint.

**Step 2: Add `#include "sk_unique_ptr"` to device.cpp**

**Step 3: Build and run tests**

Run: `cmake --preset build_x86_64 && cd build_x86_64 && make unit-test`

**Step 4: Commit**

```bash
git add src/device/device.cpp
git commit --signoff -m "refactor(device): use unique_ptr for DMA buffer allocation safety

Use make_unique for IoBuffer creation with automatic cleanup on
allocation failure, release() to transfer to DeviceNode raw field."
```

---

### Task 5: Batch 3 — Replace VFS Dentry/File with unique_ptr

**Files:**
- Modify: `src/filesystem/vfs/open.cpp:76,100` — Dentry and File creation
- Modify: `src/filesystem/vfs/lookup.cpp:103` — Dentry creation
- Modify: `src/filesystem/vfs/mount.cpp:65,91,146` — Dentry creation, delete
- Modify: `src/filesystem/vfs/mkdir.cpp:66` — Dentry creation
- Modify: `src/filesystem/vfs/close.cpp:26` — delete file
- Modify: `src/filesystem/vfs/rmdir.cpp:77` — delete target_dentry
- Modify: `src/filesystem/vfs/unlink.cpp:76` — delete target_dentry

**Strategy**: VFS Dentry/File objects are created with `new` and stored into linked structures (parent-child via `AddChild`). The ownership is transferred to the parent dentry. Since the Dentry/File types are C-style structs with raw pointer linkage, we use `make_unique` for safe construction and `.release()` when transferring ownership to the linked structure.

For `delete` sites (close.cpp, rmdir.cpp, unlink.cpp, mount.cpp), the code is removing objects from the linked structure and then deleting. We can wrap in a temporary `unique_ptr` for RAII safety, or simply keep the `delete` as-is since the pattern is `RemoveChild + delete` in sequence.

**Pragmatic approach**: Use `make_unique` for creation → `.release()` after `AddChild`. For deletion, keep existing `delete` (ownership is already clear in context).

**Step 1: Update open.cpp**

Add `#include "sk_unique_ptr"`.

Lines 76-84: Change `new Dentry()` to use unique_ptr:
```cpp
auto new_dentry = sk_std::make_unique<Dentry>();
if (!new_dentry) {
  return std::unexpected(Error(ErrorCode::kOutOfMemory));
}
strncpy(new_dentry->name, file_name, sizeof(new_dentry->name) - 1);
new_dentry->name[sizeof(new_dentry->name) - 1] = '\0';
new_dentry->inode = create_result.value();
dentry = new_dentry.get();
AddChild(parent_dentry, new_dentry.release());
```

Lines 100-103: Change `new File()` to use unique_ptr:
```cpp
auto new_file = sk_std::make_unique<File>();
if (!new_file) {
  return std::unexpected(Error(ErrorCode::kOutOfMemory));
}
File* file = new_file.release();
```

**Step 2: Update lookup.cpp**

Add `#include "sk_unique_ptr"`.

Lines 103-111: Change `new Dentry()`:
```cpp
auto new_child = sk_std::make_unique<Dentry>();
if (!new_child) {
  return std::unexpected(Error(ErrorCode::kOutOfMemory));
}
strncpy(new_child->name, component, sizeof(new_child->name) - 1);
new_child->name[sizeof(new_child->name) - 1] = '\0';
new_child->inode = result.value();
child = new_child.get();
AddChild(current, new_child.release());
```

**Step 3: Update mount.cpp**

Add `#include "sk_unique_ptr"`.

Line 65: Change `new Dentry()`:
```cpp
auto root_dentry_ptr = sk_std::make_unique<Dentry>();
if (!root_dentry_ptr) {
  fs->Unmount();
  return std::unexpected(Error(ErrorCode::kOutOfMemory));
}
Dentry* root_dentry = root_dentry_ptr.release();
```

Lines 91, 146: Keep `delete root_dentry` / `delete mp->root_dentry` as-is (teardown path, ownership is clear).

**Step 4: Update mkdir.cpp**

Add `#include "sk_unique_ptr"`.

Lines 66-73:
```cpp
auto new_dentry = sk_std::make_unique<Dentry>();
if (!new_dentry) {
  return std::unexpected(Error(ErrorCode::kOutOfMemory));
}
strncpy(new_dentry->name, dir_name, sizeof(new_dentry->name) - 1);
new_dentry->inode = result.value();
AddChild(parent_dentry, new_dentry.release());
```

**Step 5: close.cpp, rmdir.cpp, unlink.cpp — keep delete as-is**

These files have `delete file`, `delete target_dentry` in clear ownership-transfer contexts (already removed from parent). Keep as-is for now — unique_ptr improves construction safety, not teardown.

**Step 6: Build and run tests**

Run: `cmake --preset build_x86_64 && cd build_x86_64 && make unit-test`

**Step 7: Commit**

```bash
git add src/filesystem/vfs/open.cpp src/filesystem/vfs/lookup.cpp \
        src/filesystem/vfs/mount.cpp src/filesystem/vfs/mkdir.cpp
git commit --signoff -m "refactor(vfs): use make_unique for Dentry/File allocation safety

Use unique_ptr for construction of Dentry and File objects, releasing
ownership to linked structures via release(). Prevents leaks on
early-return error paths."
```

---

### Task 6: Batch 4 — Replace task_table_ and task layer

This is the highest-risk batch. `task_table_` owns TCBs, but many functions hold raw pointers to TCBs (schedulers, per_cpu running_task, etc.).

**Files:**
- Modify: `src/task/include/task_manager.hpp:199` — change `task_table_` type
- Modify: `src/task/include/task_manager.hpp:74` — change `AddTask` signature
- Modify: `src/task/task_manager.cpp:55` — idle task creation
- Modify: `src/task/task_manager.cpp:68-128` — AddTask implementation
- Modify: `src/task/task_manager.cpp:132-136` — FindTask (return raw .get())
- Modify: `src/task/task_manager.cpp:143-164` — ReapTask
- Modify: `src/task/task_manager.cpp:178-188` — ReparentChildren
- Modify: `src/task/task_manager.cpp:191-204` — GetThreadGroup
- Modify: `src/task/clone.cpp:42,133,157,196-200` — child creation, cleanup, task_table insertion
- Modify: `src/task/wait.cpp:25-91` — target lookup, cleanup

**Strategy**:

1. Change `task_table_` to `sk_std::unordered_map<Pid, sk_std::unique_ptr<TaskControlBlock>>`.
2. `AddTask()` keeps `TaskControlBlock*` parameter for now — we store it as `unique_ptr` inside the function by wrapping: `task_table_[pid] = sk_std::unique_ptr<TaskControlBlock>(task)`.
3. `FindTask()` returns `.get()` (non-owning).
4. `ReapTask()` — erase from map auto-deletes (no manual `delete`).
5. `Clone()` — creates TCB with `new`, inserts into table via `unique_ptr`.
6. `Wait()` — uses `.release()` or erase to destroy.

**IMPORTANT**: Because we use `sk_std::unordered_map` (not `std::unordered_map`), we need to verify that `sk_std::unordered_map` supports `unique_ptr` as a value type (move-only). Check `sk_unordered_map` implementation.

If `sk_std::unordered_map` does not support move-only value types, we cannot do this batch. Fall back to keeping raw pointers in task_table_.

**Step 1: Verify sk_std::unordered_map supports move-only types**

Check `src/libcxx/include/sk_unordered_map` to see if `operator[]` or `insert`/`emplace` supports move semantics.

**Step 2: If supported, proceed with changes. If not, skip this batch or add move support to sk_unordered_map first.**

**Step 3: Update task_manager.hpp**

Add `#include "sk_unique_ptr"`.

Line 199:
```cpp
sk_std::unordered_map<Pid, sk_std::unique_ptr<TaskControlBlock>> task_table_;
```

**Step 4: Update task_manager.cpp — AddTask**

Line 85 changes from:
```cpp
task_table_[task->pid] = task;
```
to:
```cpp
task_table_[task->pid] = sk_std::unique_ptr<TaskControlBlock>(task);
```

**Step 5: Update task_manager.cpp — FindTask**

Line 135 changes from:
```cpp
return (it != task_table_.end()) ? it->second : nullptr;
```
to:
```cpp
return (it != task_table_.end()) ? it->second.get() : nullptr;
```

**Step 6: Update task_manager.cpp — ReapTask**

Lines 156-163: Remove manual delete, just erase from map:
```cpp
{
  LockGuard lock_guard{task_table_lock_};
  task_table_.erase(task->pid);
}
// unique_ptr auto-deletes on erase

klog::Debug("ReapTask: Task %zu resources freed\n", task->pid);
```

Note: The current code logs `task->pid` AFTER `delete task` — that's a use-after-free bug! With unique_ptr, we should capture the pid before erasing:
```cpp
Pid pid = task->pid;
{
  LockGuard lock_guard{task_table_lock_};
  task_table_.erase(pid);
}
klog::Debug("ReapTask: Task %zu resources freed\n", pid);
```

**Step 7: Update ReparentChildren**

Line 178-188: `task` in the loop is now `unique_ptr<TaskControlBlock>&`, so access members via `task->`:
```cpp
for (auto& [pid, task] : task_table_) {
  if (task && task->parent_pid == parent->pid) {
    task->parent_pid = kInitPid;
    ...
  }
}
```
The `task &&` check works via `unique_ptr::operator bool()`, and `task->` works via `operator->()`. Minimal changes needed.

**Step 8: Update GetThreadGroup**

Line 198-199: Change `task` access:
```cpp
if (task && task->tgid == tgid) {
  result.push_back(task.get());
}
```

**Step 9: Update clone.cpp**

Line 199: Change task_table insertion:
```cpp
task_table_[new_pid] = sk_std::unique_ptr<TaskControlBlock>(child);
```

Lines 133, 157: Change `delete child` to use unique_ptr for cleanup:
```cpp
sk_std::unique_ptr<TaskControlBlock> cleanup(child);
// unique_ptr auto-deletes
return std::unexpected(Error(...));
```

Or simpler — since child is not yet in task_table_, just keep `delete child` as-is (ownership hasn't been transferred yet).

**Step 10: Update wait.cpp**

Lines 83-91: Change cleanup pattern. Currently:
```cpp
LockGuard lock_guard(task_table_lock_);
auto it = task_table_.find(target->pid);
task_table_.erase(it->first);
...
delete target;
```

Change to:
```cpp
Pid result_pid = target->pid;
if (status) {
  *status = target->exit_code;
}

{
  LockGuard lock_guard(task_table_lock_);
  task_table_.erase(result_pid);  // auto-deletes TCB via unique_ptr
}

klog::Debug("Wait: pid=%zu reaped child=%zu\n", current->pid, result_pid);
return result_pid;
```

**Step 11: Build and run tests**

Run: `cmake --preset build_x86_64 && cd build_x86_64 && make unit-test`

**Step 12: Commit**

```bash
git add src/task/include/task_manager.hpp src/task/task_manager.cpp \
        src/task/clone.cpp src/task/wait.cpp
git commit --signoff -m "refactor(task): use unique_ptr for TaskControlBlock ownership in task_table_

task_table_ now owns TCBs via unique_ptr. Eliminates manual delete in
ReapTask and Wait. Fixes use-after-free in ReapTask debug log."
```

---

### Task 7: Batch 5 — Replace main.cpp task creation

**Files:**
- Modify: `src/main.cpp:89-96` — use make_unique + release()

**Step 1: Add `#include "sk_unique_ptr"` to main.cpp**

**Step 2: Change create_test_tasks()**

Lines 89-96 currently:
```cpp
auto task1 = new TaskControlBlock("Task1-Exit", 10, task1_func, ...);
auto task2 = new TaskControlBlock("Task2-Yield", 10, task2_func, ...);
auto task3 = new TaskControlBlock("Task3-Sync", 10, task3_func, ...);
auto task4 = new TaskControlBlock("Task4-Sleep", 10, task4_func, ...);
```

Change to:
```cpp
auto task1 = sk_std::make_unique<TaskControlBlock>("Task1-Exit", 10, task1_func,
                                    reinterpret_cast<void*>(0x1111));
auto task2 = sk_std::make_unique<TaskControlBlock>("Task2-Yield", 10, task2_func,
                                    reinterpret_cast<void*>(0x2222));
auto task3 = sk_std::make_unique<TaskControlBlock>("Task3-Sync", 10, task3_func,
                                    reinterpret_cast<void*>(0x3333));
auto task4 = sk_std::make_unique<TaskControlBlock>("Task4-Sleep", 10, task4_func,
                                    reinterpret_cast<void*>(0x4444));
```

And update AddTask calls to release ownership:
```cpp
task1->cpu_affinity = (1UL << core_id);
task2->cpu_affinity = (1UL << core_id);
task3->cpu_affinity = (1UL << core_id);
task4->cpu_affinity = (1UL << core_id);

tm.AddTask(task1.release());
tm.AddTask(task2.release());
tm.AddTask(task3.release());
tm.AddTask(task4.release());
```

**Step 3: Build and run tests**

Run: `cmake --preset build_x86_64 && cd build_x86_64 && make unit-test`

**Step 4: Commit**

```bash
git add src/main.cpp
git commit --signoff -m "refactor(main): use make_unique for test task creation

Use unique_ptr for TaskControlBlock construction, releasing ownership
to AddTask. Ensures cleanup on early failure paths."
```

---

### Task 8: Final verification

**Step 1: Run full test suite**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

**Step 2: Build for all architectures**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel
cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

**Step 3: Run pre-commit**

```bash
pre-commit run --all-files
```

**Step 4: Verify no remaining raw new/delete for owned objects**

Search for `new TaskControlBlock`, `new Dentry`, `new File`, `new FifoScheduler`, etc. and ensure all creation sites use unique_ptr/make_unique.
