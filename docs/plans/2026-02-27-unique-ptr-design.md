# sk_std::unique_ptr Design

**Date**: 2026-02-27
**Status**: Approved
**Scope**: `src/libcxx/include/sk_unique_ptr`, unit tests, project-wide replacement of raw `new`/`delete`

## Goal

Implement `sk_std::unique_ptr` — a move-only owning smart pointer — in the kernel's freestanding C++ runtime. After thorough testing, apply it across all raw `new`/`delete` ownership sites in the project.

## Constraints

- Freestanding C++23, no RTTI, no exceptions
- Usable only after `MemoryInit()` (same as `sk_std::shared_ptr`)
- Must follow existing `sk_std` conventions: file naming (`sk_unique_ptr`), namespace (`sk_std`), Doxygen, trailing return types
- Google C++ style via `.clang-format`

## API Surface

### Primary Template: `unique_ptr<T, Deleter>`

```cpp
namespace sk_std {

template <typename T>
struct default_delete {
  constexpr default_delete() noexcept = default;
  void operator()(T* ptr) const noexcept { delete ptr; }
};

template <typename T, typename Deleter = default_delete<T>>
class unique_ptr {
 public:
  constexpr unique_ptr() noexcept;
  explicit unique_ptr(T* ptr) noexcept;
  unique_ptr(T* ptr, const Deleter& d) noexcept;
  unique_ptr(unique_ptr&& other) noexcept;
  unique_ptr(const unique_ptr&) = delete;
  ~unique_ptr();

  auto operator=(unique_ptr&& other) noexcept -> unique_ptr&;
  auto operator=(const unique_ptr&) -> unique_ptr& = delete;
  auto operator=(decltype(nullptr)) noexcept -> unique_ptr&;

  [[nodiscard]] auto release() noexcept -> T*;
  auto reset(T* ptr = nullptr) noexcept -> void;
  auto swap(unique_ptr& other) noexcept -> void;

  [[nodiscard]] auto get() const noexcept -> T*;
  [[nodiscard]] auto get_deleter() noexcept -> Deleter&;
  [[nodiscard]] auto get_deleter() const noexcept -> const Deleter&;
  [[nodiscard]] explicit operator bool() const noexcept;
  [[nodiscard]] auto operator*() const noexcept -> T&;
  [[nodiscard]] auto operator->() const noexcept -> T*;

 private:
  T* ptr_;
  Deleter deleter_;
};

template <typename T, typename... Args>
auto make_unique(Args&&... args) -> unique_ptr<T>;

}  // namespace sk_std
```

### Array Specialization: `unique_ptr<T[]>`

```cpp
template <typename T>
struct default_delete<T[]> {
  constexpr default_delete() noexcept = default;
  void operator()(T* ptr) const noexcept { delete[] ptr; }
};

template <typename T, typename Deleter>
class unique_ptr<T[], Deleter> {
  // Same as primary template except:
  // - No operator* or operator->
  // - Adds operator[](size_t idx) -> T&
};
```

### Non-member Utilities

```cpp
template <typename T, typename D>
auto swap(unique_ptr<T, D>& a, unique_ptr<T, D>& b) noexcept -> void;

template <typename T1, typename D1, typename T2, typename D2>
auto operator==(const unique_ptr<T1,D1>&, const unique_ptr<T2,D2>&) -> bool;

template <typename T, typename D>
auto operator==(const unique_ptr<T,D>&, decltype(nullptr)) -> bool;
```

## Ownership Transfer Strategy

**Rule**: Functions that create and transfer ownership accept/return `unique_ptr`. Functions that borrow (non-owning) keep raw `T*` via `.get()`.

| Current API | New API | Reason |
|------------|---------|--------|
| `AddTask(TaskControlBlock*)` | `AddTask(unique_ptr<TaskControlBlock>)` | TaskManager takes ownership |
| `FindTask(Pid)` → `TaskControlBlock*` | Unchanged | Non-owning lookup |
| `GetCurrentTask()` → `TaskControlBlock*` | Unchanged | Non-owning observer |
| `SchedulerBase::Enqueue(TaskControlBlock*)` | Unchanged | Scheduler borrows, doesn't own |

## Data Structure Changes

```cpp
// task_manager.hpp — task_table_ owns TCBs
sk_std::unordered_map<Pid, sk_std::unique_ptr<TaskControlBlock>> task_table_;

// task_manager.hpp — CpuSchedData owns schedulers
std::array<sk_std::unique_ptr<SchedulerBase>, SchedPolicy::kPolicyCount> schedulers{};

// device.cpp — DeviceNode owns IoBuffer
sk_std::unique_ptr<IoBuffer> dma_buffer;
```

## Replacement Batches (by risk)

1. **Schedulers** — `CpuSchedData::schedulers[]`, simplest ownership
2. **IoBuffer** — `device.cpp`, simple create/destroy
3. **VFS Dentry/File** — parent/child relationships, use `.get()` for non-owning refs
4. **Task layer** — `task_table_`, `Clone`, `ReapTask`, `Wait`
5. **main.cpp** — task creation, depends on batch 4 API

## Testing Plan

Unit tests in `tests/unit_test/sk_unique_ptr_test.cpp`:
1. Default construction (null state)
2. Owning construction + destructor invocation
3. Move construction + move assignment
4. `release()`, `reset()`, `swap()`
5. `operator*`, `operator->`, `operator bool`
6. `nullptr` assignment
7. Custom deleter
8. Array specialization: `operator[]`, `delete[]`
9. `make_unique` factory
10. Non-member `swap` and comparison operators
11. Polymorphic types (base pointer managing derived)
