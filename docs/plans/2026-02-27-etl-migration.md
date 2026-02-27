# ETL Migration Plan — Pool-Backed `kstd::` Containers (No `generic_pool`, Centralized Pools)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace `sk_std::` containers with pool-backed implementations that use ETL infrastructure internally, exposed through a `kstd::` namespace. No `etl::` appears in kernel code. No `etl::generic_pool`. All pools centralized in one file.

**Architecture:** Hybrid approach — use ETL `_ext` container variants (`list_ext`, `vector_ext`) where native pool support exists; keep custom implementation with pool-backed allocation for `unordered_map` (no ETL `_ext` variant). All containers live in `kstd::` namespace. Memory comes from `etl::pool<T,N>` (typed pools only — never `etl::generic_pool`). All pool instances live in `kstd_pools.hpp`.

**Tech Stack:** ETL 20.45.0 (`3rd/etl`), C++23, CMake, freestanding kernel.

**Build/Test Commands:**
```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel    # Build kernel
make run                                  # Run in QEMU
```

---

## Three Constraints (Non-Negotiable)

1. **No `etl::generic_pool`** — Only `etl::pool<T,N>` (typed pools). `etl::pool<T,N>` inherits from `generic_pool` internally, but we never instantiate `generic_pool` directly.
2. **All pools in one place** — Every pool instance is declared in `src/libcxx/include/kstd_pools.hpp`. Consumer code references pools from there, never creates its own.
3. **No `etl::` in kernel code** — All ETL types are wrapped in `kstd::` aliases/wrappers. Only `kstd_pool.hpp`, `kstd_vector`, `kstd_list`, and `kstd_pools.hpp` include ETL headers. Consumer `.hpp`/`.cpp` files never `#include "etl/..."`.

---

## Design Overview

### Container → ETL Mapping

| kstd type | Backing | ETL component | Notes |
|-----------|---------|---------------|-------|
| `kstd::vector<T>` | `etl::vector_ext<T>` | External buffer from typed pool | Buffer from `kstd::Pool<T,N>` |
| `kstd::list<T>` | `etl::list_ext<T>` | Takes `kstd::PoolRef&` directly | Native shared pool support |
| `kstd::unordered_map<K,V>` | Custom (keep sk_std impl) | Node alloc via `kstd::PoolRef` | No ETL `_ext` variant exists |
| `kstd::unique_ptr<T>` | Keep current sk_std impl | Unchanged | Smart pointer, not pool-backed |
| `kstd::priority_queue<T>` | Adapter over `kstd::vector` | Inherits vector's pool backing | Thin adapter |

### Memory Architecture

```
┌──────────────────────────────────────────┐
│            kstd:: namespace              │  ← Consumer code sees ONLY this
│  vector<T>  list<T>  unordered_map<K,V>  │
│  unique_ptr<T>  priority_queue<T>        │
├──────────────────────────────────────────┤
│         kstd_pools.hpp                   │  ← ALL pool instances here
│  TaskPtrVectorPool, ListNodePool, etc.   │
├──────────────────────────────────────────┤
│         kstd_pool.hpp                    │  ← Pool type aliases
│  kstd::Pool<T,N> = etl::pool<T,N>       │
│  kstd::PoolRef   = etl::ipool           │
├──────────────────────────────────────────┤
│          etl::ipool / etl::pool<T,N>     │  ← NEVER visible to consumers
│         (static storage, no heap)        │
└──────────────────────────────────────────┘
```

### File Layout

```
src/libcxx/include/
├── kstd_pool.hpp          # Pool type aliases (kstd::Pool, kstd::PoolRef)
├── kstd_pools.hpp         # ALL pool instances (centralized registry)
├── kstd_vector            # kstd::vector<T> — wraps etl::vector_ext<T>
├── kstd_list              # kstd::list<T> — wraps etl::list_ext<T>
├── kstd_unordered_map     # kstd::unordered_map<K,V> — pool-backed custom impl
├── kstd_unique_ptr        # kstd::unique_ptr<T> — renamed from sk_std
├── kstd_priority_queue    # kstd::priority_queue<T> — adapter over kstd::vector
├── sk_vector              # DEPRECATED: includes kstd_vector + using alias
├── sk_list                # DEPRECATED: includes kstd_list + using alias
├── ...                    # Other sk_ headers: compatibility shims
```

### Namespace Compatibility Strategy

Each old `sk_*` header becomes a thin shim:
```cpp
// sk_vector (compatibility shim)
#include "kstd_vector"
namespace sk_std {
  using kstd::vector;
}
```

This allows incremental migration — consumers keep working, then we update `#include` directives and namespaces one file at a time.

---

## Consumer Inventory

### Heavy Consumers (Task Subsystem)

| File | sk_std:: usage |
|------|---------------|
| `src/task/include/task_manager.hpp` | `unique_ptr`, `unordered_map`×3, `list`, `vector`, `priority_queue` |
| `src/task/include/cfs_scheduler.hpp` | `vector`, `priority_queue` |
| `src/task/include/fifo_scheduler.hpp` | `list` |
| `src/task/include/rr_scheduler.hpp` | `list` |
| `src/task/task_manager.cpp` | `unique_ptr`, `vector` |

### Light Consumers (unique_ptr only)

| File | sk_std:: usage |
|------|---------------|
| `src/main.cpp` | `make_unique` |
| `src/device/device.cpp` | `make_unique` |
| `src/filesystem/vfs/mount.cpp` | `make_unique` |
| `src/filesystem/vfs/lookup.cpp` | `make_unique` |
| `src/filesystem/vfs/open.cpp` | `make_unique`×2 |
| `src/filesystem/vfs/mkdir.cpp` | `make_unique` |

### Other Consumers

| File | sk_std:: usage |
|------|---------------|
| `src/arch/*/arch_main.cpp` | `sk_std::` (vector/list) |
| `src/include/kernel_log.hpp` | `sk_std::` |
| `src/include/basic_info.hpp` | `sk_std::` |
| `src/include/kernel_fdt.hpp` | `sk_std::` |
| `src/device/include/device_node.hpp` | `sk_std::` |

### Unused sk_ Headers (0 consumers — delete)

- `sk_shared_ptr`, `sk_set`, `sk_rb_tree`, `sk_queue`

### NOT Migrated (stays sk_std)

- `sk_cstdio`, `sk_cstring`, `sk_iostream`, `sk_cassert`, `sk_libcxx.h` — these are C/C++ runtime wrappers, not containers.

---

## Task 0: ETL Build Integration

**Files:**
- Modify: `cmake/compile_config.cmake` (add `etl::etl` to kernel_link_libraries)
- Create: `src/include/etl_profile.h` (ETL configuration for freestanding kernel)

**Step 1: Create ETL profile header**

ETL requires an `etl_profile.h` visible on the include path. For a freestanding kernel:

```cpp
// src/include/etl_profile.h
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief ETL configuration for SimpleKernel freestanding environment.
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_ETL_PROFILE_H_
#define SIMPLEKERNEL_SRC_INCLUDE_ETL_PROFILE_H_

// Use generic C++23 profile as base
#define ETL_CPP23_SUPPORTED 1

// No exceptions in kernel
#define ETL_NO_CHECKS 1

// No STL available in freestanding
#define ETL_NO_STL 1

#endif  // SIMPLEKERNEL_SRC_INCLUDE_ETL_PROFILE_H_
```

**Step 2: Add `etl::etl` to kernel link libraries**

In `cmake/compile_config.cmake`, add `etl::etl` to the `kernel_link_libraries` target:

```cmake
TARGET_LINK_LIBRARIES (
    kernel_link_libraries
    INTERFACE link_libraries
              kernel_compile_definitions
              kernel_compile_options
              kernel_link_options
              nanoprintf-lib
              dtc-lib
              cpu_io
              bmalloc
              MPMCQueue
              device_framework
              fatfs_lib
              etl::etl
              gcc
              ...)
```

**Step 3: Verify ETL compiles with kernel**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel
```
Expected: Build succeeds with no ETL-related errors.

**Step 4: Commit**

```bash
git add src/include/etl_profile.h cmake/compile_config.cmake
git commit --signoff -m "build(etl): add etl::etl to kernel link libraries and create etl_profile.h"
```

---

## Task 1: Create Pool Infrastructure (`kstd_pool.hpp`)

**Files:**
- Create: `src/libcxx/include/kstd_pool.hpp`

This is the foundation. All pool-backed containers depend on this. Only typed pools — NO `generic_pool`.

**Step 1: Write `kstd_pool.hpp`**

```cpp
// src/libcxx/include/kstd_pool.hpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Pool infrastructure for kstd containers.
 *
 * Provides pool-backed allocation using ETL's ipool/pool<T,N>.
 * All ETL details are hidden from consumers.
 *
 * @note Only typed pools (etl::pool<T,N>) are used.
 *       etl::generic_pool is NOT used anywhere.
 */

#ifndef SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_POOL_HPP_
#define SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_POOL_HPP_

#include <cstddef>

#include "etl/ipool.h"
#include "etl/pool.h"

namespace kstd {

/// @brief Type-erased pool reference. Consumers see this, not etl::ipool.
using PoolRef = etl::ipool;

/// @brief Fixed-capacity typed object pool with static storage.
/// @tparam T Element type.
/// @tparam N Maximum number of elements.
template <typename T, size_t N>
using Pool = etl::pool<T, N>;

}  // namespace kstd

#endif  // SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_POOL_HPP_
```

**Step 2: Verify it compiles**

Header-only. Include it from any .cpp and build:
```bash
cd build_riscv64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/libcxx/include/kstd_pool.hpp
git commit --signoff -m "feat(kstd): add pool infrastructure header kstd_pool.hpp"
```

---

## Task 2: Create `kstd::vector<T>` (wrapping `etl::vector_ext<T>`)

**Files:**
- Create: `src/libcxx/include/kstd_vector`
- Modify: `src/libcxx/include/sk_vector` (make it a compatibility shim)

**Step 1: Write `kstd_vector`**

`etl::vector_ext<T>` takes a `void* buffer` and `size_t max_size` in its constructor. We alias this.

```cpp
// src/libcxx/include/kstd_vector
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Pool-backed vector using ETL vector_ext internally.
 *
 * kstd::vector<T> wraps etl::vector_ext<T> with buffer management.
 * The buffer is provided externally (from a pool or static storage).
 *
 * @note ETL vector_ext requires the buffer to be provided at construction.
 *       This wrapper does NOT own the buffer — the caller must ensure the
 *       buffer outlives the vector.
 */

#ifndef SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_VECTOR_
#define SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_VECTOR_

#include <cstddef>

#include "etl/vector.h"

namespace kstd {

/// @brief Vector with externally-provided storage buffer.
///
/// This is a thin alias over etl::vector_ext<T>. The buffer must be
/// provided at construction time and must outlive the vector.
///
/// Usage:
/// @code
///   alignas(int) uint8_t buf[sizeof(int) * 64];
///   kstd::vector<int> v(buf, 64);
///   v.push_back(42);
/// @endcode
///
/// Or with a fixed-capacity convenience:
/// @code
///   kstd::static_vector<int, 64> v;
///   v.push_back(42);
/// @endcode
template <typename T>
using vector = etl::vector_ext<T>;

/// @brief Fixed-capacity vector with internal static storage.
///
/// Convenience alias for when you know the max capacity at compile time.
/// No external buffer management needed.
template <typename T, size_t N>
using static_vector = etl::vector<T, N>;

/// @brief Type-erased vector interface.
///
/// Useful for functions that accept vectors of any capacity.
template <typename T>
using ivector = etl::ivector<T>;

}  // namespace kstd

#endif  // SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_VECTOR_
```

**Step 2: Convert `sk_vector` to compatibility shim**

```cpp
// src/libcxx/include/sk_vector
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @deprecated Use kstd_vector and kstd::vector<T> instead.
 */

#ifndef SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_VECTOR_
#define SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_VECTOR_

#include "kstd_vector"

namespace sk_std {
template <typename T>
using vector = kstd::vector<T>;

template <typename T, size_t N>
using static_vector = kstd::static_vector<T, N>;
}  // namespace sk_std

#endif  // SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_VECTOR_
```

**Step 3: Build and verify**

```bash
cd build_riscv64 && make SimpleKernel
```

Expected: Build will **FAIL** because `sk_std::vector` API differs from `etl::vector_ext<T>` API.
- Old: `sk_std::vector<T>()` default-constructs with no args, heap-allocates on `push_back`.
- New: `etl::vector_ext<T>(void* buf, size_t max)` requires a buffer at construction.

Consumer code must be updated. See Tasks 6-8 for consumer migration.

**Step 4: Commit**

```bash
git add src/libcxx/include/kstd_vector src/libcxx/include/sk_vector
git commit --signoff -m "feat(kstd): add kstd::vector wrapping etl::vector_ext"
```

---

## Task 3: Create `kstd::list<T>` (wrapping `etl::list_ext<T>`)

**Files:**
- Create: `src/libcxx/include/kstd_list`
- Modify: `src/libcxx/include/sk_list` (make it a compatibility shim)

**Step 1: Write `kstd_list`**

`etl::list_ext<T>` takes an `etl::ipool&` in its constructor. This is the cleanest mapping.

```cpp
// src/libcxx/include/kstd_list
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Pool-backed doubly-linked list using ETL list_ext internally.
 *
 * kstd::list<T> wraps etl::list_ext<T> which takes a kstd::PoolRef&
 * for node allocation. Multiple lists can share the same pool.
 *
 * Usage:
 * @code
 *   // Pool type is etl::ilist<T>::data_node_t
 *   kstd::Pool<kstd::list_node_t<int>, 64> pool;
 *   kstd::list<int> my_list(pool);
 *   my_list.push_back(42);
 * @endcode
 */

#ifndef SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_LIST_
#define SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_LIST_

#include <cstddef>

#include "etl/list.h"

#include "kstd_pool.hpp"

namespace kstd {

/// @brief Doubly-linked list with externally-provided node pool.
///
/// Requires a kstd::PoolRef& at construction for node allocation.
template <typename T>
using list = etl::list_ext<T>;

/// @brief Fixed-capacity list with internal static storage.
template <typename T, size_t N>
using static_list = etl::list<T, N>;

/// @brief Type-erased list interface.
template <typename T>
using ilist = etl::ilist<T>;

/// @brief Node type for list<T> pools.
///
/// Use this to declare pools for kstd::list<T>:
///   kstd::Pool<kstd::list_node_t<int>, 64> pool;
template <typename T>
using list_node_t = typename etl::ilist<T>::data_node_t;

}  // namespace kstd

#endif  // SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_LIST_
```

**Step 2: Convert `sk_list` to compatibility shim**

```cpp
// src/libcxx/include/sk_list
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @deprecated Use kstd_list and kstd::list<T> instead.
 */

#ifndef SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_LIST_
#define SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_LIST_

#include "kstd_list"

namespace sk_std {
template <typename T>
using list = kstd::list<T>;

template <typename T, size_t N>
using static_list = kstd::static_list<T, N>;
}  // namespace sk_std

#endif  // SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_LIST_
```

**Step 3: Commit**

```bash
git add src/libcxx/include/kstd_list src/libcxx/include/sk_list
git commit --signoff -m "feat(kstd): add kstd::list wrapping etl::list_ext"
```

---

## Task 4: Create `kstd::unique_ptr<T>` and `kstd::priority_queue<T>`

**Files:**
- Create: `src/libcxx/include/kstd_unique_ptr`
- Create: `src/libcxx/include/kstd_priority_queue`
- Modify: `src/libcxx/include/sk_unique_ptr` (shim)
- Modify: `src/libcxx/include/sk_priority_queue` (shim)

**Step 1: Write `kstd_unique_ptr`**

`unique_ptr` doesn't benefit from ETL pools — it's a smart pointer that wraps any allocation. Copy the existing `sk_unique_ptr` implementation, replacing `namespace sk_std` → `namespace kstd` and updating the header guard.

```cpp
// src/libcxx/include/kstd_unique_ptr
// Copy the full content of sk_unique_ptr, replacing:
//   namespace sk_std → namespace kstd
//   header guard → SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_UNIQUE_PTR_
```

**Step 2: Write `kstd_priority_queue`**

Priority queue is an adapter. It currently wraps `sk_std::vector`. Update it to use `kstd::vector` (which is `etl::vector_ext<T>`). Since `etl::vector_ext<T>` requires a buffer, `priority_queue` must also take a buffer reference in construction.

```cpp
// src/libcxx/include/kstd_priority_queue
// Copy sk_priority_queue, replace sk_std::vector with kstd::vector,
// update namespace to kstd, update constructor to accept buffer.
// Include "kstd_vector" instead of "sk_vector".
```

**Step 3: Convert old headers to shims**

Same pattern as Tasks 2-3: include kstd header, add `using` alias in `sk_std::`.

**Step 4: Commit**

```bash
git add src/libcxx/include/kstd_unique_ptr src/libcxx/include/kstd_priority_queue \
        src/libcxx/include/sk_unique_ptr src/libcxx/include/sk_priority_queue
git commit --signoff -m "feat(kstd): add kstd::unique_ptr and kstd::priority_queue"
```

---

## Task 5: Create `kstd::unordered_map<K,V>` (pool-backed custom impl)

**Files:**
- Create: `src/libcxx/include/kstd_unordered_map`
- Modify: `src/libcxx/include/sk_unordered_map` (shim)

ETL has no `unordered_map_ext` variant. We keep the current `sk_std::unordered_map` implementation but replace `::operator new/delete` with pool-backed allocation via `kstd::PoolRef`.

**Step 1: Write `kstd_unordered_map`**

Copy `sk_unordered_map` into `kstd_unordered_map` with these changes:
1. Namespace `sk_std` → `kstd`
2. Replace `new Node(...)` calls with pool-based `create<Node>(...)` via a `kstd::PoolRef&` member
3. Replace `delete node` calls with pool-based `destroy<Node>(node)` via the pool
4. Replace bucket array allocation (`::operator new`) with buffer provided at construction
5. Constructor takes pool references for node and bucket allocation

Key changes in the implementation:
```cpp
namespace kstd {

template <typename Key, typename Value, typename Hash = ..., typename KeyEqual = ...>
class unordered_map {
 public:
  // Constructor takes a node pool and bucket buffer
  unordered_map(PoolRef& node_pool, void* bucket_buffer, size_t bucket_count)
      : node_pool_(node_pool),
        buckets_(static_cast<Node**>(bucket_buffer)),
        bucket_count_(bucket_count),
        size_(0) {
    for (size_t i = 0; i < bucket_count_; ++i) {
      buckets_[i] = nullptr;
    }
  }

 private:
  // Replace: Node* new_node = new Node(k, v);
  // With:    Node* new_node = node_pool_.create<Node>(k, v);

  // Replace: delete node;
  // With:    node_pool_.destroy<Node>(node);

  PoolRef& node_pool_;     // Pool for node allocation
  Node** buckets_;          // Externally provided bucket array
  size_t bucket_count_;
  size_t size_;
};

}  // namespace kstd
```

**Step 2: Convert `sk_unordered_map` to shim**

Same pattern as other headers.

**Step 3: Commit**

```bash
git add src/libcxx/include/kstd_unordered_map src/libcxx/include/sk_unordered_map
git commit --signoff -m "feat(kstd): add kstd::unordered_map with pool-backed node allocation"
```

---

## Task 5.5: Create Centralized Pool Registry (`kstd_pools.hpp`)

**Files:**
- Create: `src/libcxx/include/kstd_pools.hpp`

**This is the key constraint file.** ALL pool instances in the kernel live here. Consumer code includes this header and references pools by name.

**Step 1: Write `kstd_pools.hpp`**

```cpp
// src/libcxx/include/kstd_pools.hpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Centralized pool registry for all kstd containers.
 *
 * ALL pool instances used by kernel code are declared here.
 * No consumer code should create its own etl::pool — use the pools from
 * this file instead.
 *
 * @note Pool sizes are compile-time constants. Adjust kMax* values as needed.
 */

#ifndef SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_POOLS_HPP_
#define SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_POOLS_HPP_

#include <cstdint>

#include "kstd_pool.hpp"
#include "kstd_list"

// Forward declarations for types used in pools
class TaskControlBlock;

namespace kstd::pools {

/// @name Capacity Constants
/// @{
static constexpr size_t kMaxTasks = 64;
static constexpr size_t kMaxSchedulerEntries = 64;
static constexpr size_t kMapBucketCount = 128;
/// @}

/// @name Task Subsystem Pools
/// @{

/// Pool for vector<TaskControlBlock*> buffers
/// Used by: TaskManager::tasks_, CfsScheduler, priority_queue backing
inline alignas(TaskControlBlock*)
    uint8_t task_ptr_vector_buf[sizeof(TaskControlBlock*) * kMaxTasks];

/// Pool for list<TaskControlBlock*> nodes
/// Used by: FifoScheduler, RrScheduler, TaskManager run queues
inline Pool<list_node_t<TaskControlBlock*>, kMaxTasks> task_list_node_pool;

/// Pool for unordered_map nodes (task_table_)
/// Node type depends on the final unordered_map implementation
/// TODO: Define exact node type after kstd_unordered_map is finalized

/// Bucket buffer for unordered_map
inline TaskControlBlock* map_bucket_buf[kMapBucketCount] = {};

/// @}

/// @name Scheduler Pools
/// @{

/// Pool for CFS scheduler vector buffer
inline alignas(TaskControlBlock*)
    uint8_t cfs_vector_buf[sizeof(TaskControlBlock*) * kMaxSchedulerEntries];

/// Pool for FIFO scheduler list nodes
inline Pool<list_node_t<TaskControlBlock*>, kMaxSchedulerEntries>
    fifo_list_node_pool;

/// Pool for RR scheduler list nodes
inline Pool<list_node_t<TaskControlBlock*>, kMaxSchedulerEntries>
    rr_list_node_pool;

/// @}

}  // namespace kstd::pools

#endif  // SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_POOLS_HPP_
```

**Step 2: Verify it compiles**

```bash
cd build_riscv64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/libcxx/include/kstd_pools.hpp
git commit --signoff -m "feat(kstd): add centralized pool registry kstd_pools.hpp"
```

---

## Task 6: Migrate Consumer Code — Task Subsystem

This is the largest consumer. Files:
- `src/task/include/task_manager.hpp`
- `src/task/include/cfs_scheduler.hpp`
- `src/task/include/fifo_scheduler.hpp`
- `src/task/include/rr_scheduler.hpp`
- `src/task/task_manager.cpp`
- `src/task/task_control_block.cpp`
- `src/task/schedule.cpp`

**Key Change Pattern:**

Old (heap-allocated, unlimited capacity):
```cpp
#include "sk_vector"
#include "sk_list"
#include "sk_unordered_map"
sk_std::vector<TaskControlBlock*> tasks_;
sk_std::list<TaskControlBlock*> run_queue_;
sk_std::unordered_map<uint64_t, sk_std::unique_ptr<TaskControlBlock>> task_table_;
```

New (pool-backed, fixed max capacity, pools from kstd_pools.hpp):
```cpp
#include "kstd_vector"
#include "kstd_list"
#include "kstd_unordered_map"
#include "kstd_pools.hpp"

// Vector: use buffer from centralized pools
kstd::vector<TaskControlBlock*> tasks_{
    kstd::pools::task_ptr_vector_buf, kstd::pools::kMaxTasks};

// List: use node pool from centralized pools
kstd::list<TaskControlBlock*> run_queue_{kstd::pools::task_list_node_pool};

// Unordered map: use node pool + bucket buffer from centralized pools
kstd::unordered_map<uint64_t, kstd::unique_ptr<TaskControlBlock>> task_table_{
    kstd::pools::map_node_pool, kstd::pools::map_bucket_buf,
    kstd::pools::kMapBucketCount};
```

**Step 1: Update task_manager.hpp**

This file has the heaviest usage:
- `sk_std::unordered_map` → `kstd::unordered_map` + pools from kstd_pools.hpp
- `sk_std::unique_ptr` → `kstd::unique_ptr`
- `sk_std::vector` → `kstd::vector` + buffer from kstd_pools.hpp
- `sk_std::list` → `kstd::list` + pool from kstd_pools.hpp
- `sk_std::priority_queue` → `kstd::priority_queue` + buffer from kstd_pools.hpp

**Step 2: Update schedulers**

- `cfs_scheduler.hpp`: uses `sk_std::priority_queue`, `sk_std::vector` → use buffers from kstd_pools.hpp
- `fifo_scheduler.hpp`: uses `sk_std::list` → use `kstd::pools::fifo_list_node_pool`
- `rr_scheduler.hpp`: uses `sk_std::list` → use `kstd::pools::rr_list_node_pool`

**Step 3: Update .cpp files**

- `task_manager.cpp`: update `sk_std::` → `kstd::`, adjust construction
- `task_control_block.cpp`: update if needed
- `schedule.cpp`: update if needed

**Step 4: Build and test**

```bash
cd build_riscv64 && make SimpleKernel
make run
```

**Step 5: Commit**

```bash
git add src/task/
git commit --signoff -m "refactor(task): migrate task subsystem from sk_std to kstd pool-backed containers"
```

---

## Task 7: Migrate Consumer Code — Filesystem, Device, Main

**Files:**
- `src/filesystem/vfs/mount.cpp` — uses `sk_std::unique_ptr`
- `src/filesystem/vfs/lookup.cpp` — uses `sk_std::unique_ptr`
- `src/filesystem/vfs/open.cpp` — uses `sk_std::unique_ptr`
- `src/filesystem/vfs/mkdir.cpp` — uses `sk_std::unique_ptr`
- `src/device/device.cpp` — uses `sk_std::unique_ptr`
- `src/main.cpp` — uses vector, list, unique_ptr, priority_queue

These are simpler since most only use `unique_ptr` (which is unchanged in behavior).

**Step 1: Update filesystem files**

Replace `#include "sk_unique_ptr"` → `#include "kstd_unique_ptr"` and `sk_std::unique_ptr` → `kstd::unique_ptr`.

**Step 2: Update device.cpp**

Same pattern as filesystem.

**Step 3: Update main.cpp**

`main.cpp` uses multiple container types. Update includes and namespaces. All pool/buffer storage comes from `kstd_pools.hpp`.

**Step 4: Build and test**

```bash
cd build_riscv64 && make SimpleKernel && make run
```

**Step 5: Commit**

```bash
git add src/filesystem/ src/device/ src/main.cpp
git commit --signoff -m "refactor(core): migrate filesystem, device, and main to kstd containers"
```

---

## Task 8: Migrate Remaining Consumers — Arch, Includes

**Files:**
- `src/arch/riscv64/arch_main.cpp` — uses `sk_std::`
- `src/arch/x86_64/arch_main.cpp` — uses `sk_std::`
- `src/arch/aarch64/arch_main.cpp` — uses `sk_std::`
- `src/include/kernel_log.hpp` — uses `sk_std::`
- `src/include/basic_info.hpp` — uses `sk_std::`
- `src/include/kernel_fdt.hpp` — uses `sk_std::`
- `src/device/include/device_node.hpp` — uses `sk_std::`

**Step 1: Update each file**

Replace `sk_std::` with `kstd::` and update includes. If any of these use containers (vector, list), provide pool/buffer from `kstd_pools.hpp` (add new pools there if needed).

**Step 2: Build and test**

```bash
cd build_riscv64 && make SimpleKernel && make run
```

**Step 3: Commit**

```bash
git add src/arch/ src/include/ src/device/include/
git commit --signoff -m "refactor(arch,include): migrate arch and include files to kstd"
```

---

## Task 9: Remove Old sk_std Implementations

**Files:**
- Minimize: `src/libcxx/include/sk_vector` (keep as shim or delete)
- Minimize: `src/libcxx/include/sk_list`
- Minimize: `src/libcxx/include/sk_unordered_map`
- Minimize: `src/libcxx/include/sk_unique_ptr`
- Minimize: `src/libcxx/include/sk_priority_queue`
- Delete: `sk_shared_ptr`, `sk_set`, `sk_rb_tree`, `sk_queue`

**Decision:** Keep sk_* headers as compatibility shims (5-10 lines each) OR delete them entirely if all consumers have been migrated.

**Step 1: Verify no remaining `sk_std::` usage**

```bash
grep -r "sk_std::" src/ --include="*.cpp" --include="*.hpp" --include="*.h"
```

Expected: Zero matches (or only in the shim headers themselves).

**Step 2: Delete or minimize old headers**

If shims are kept, they're 5-10 lines each. If deleted, remove from `src/libcxx/include/`.

**Step 3: Delete unused headers**

`sk_shared_ptr`, `sk_set`, `sk_rb_tree`, `sk_queue` have 0 direct uses. Safe to delete.

**Step 4: Build and test**

```bash
cd build_riscv64 && make SimpleKernel && make run
```

**Step 5: Commit**

```bash
git add -A src/libcxx/
git commit --signoff -m "refactor(libcxx): remove old sk_std implementations, keep kstd"
```

---

## Task 10: Final Verification and Cleanup

**Step 1: Full build**

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel
```

**Step 2: Run in QEMU**

```bash
make run
```

Verify kernel boots, tasks schedule, filesystem operations work.

**Step 3: Verify no etl:: leaks**

```bash
# No etl:: in consumer code (only in kstd_*.hpp headers)
grep -r "etl::" src/ --include="*.cpp" --include="*.hpp" --include="*.h" \
  | grep -v "src/libcxx/include/kstd_" | grep -v "src/include/etl_profile.h"
```

Expected: Zero matches.

**Step 4: Verify no generic_pool usage**

```bash
grep -r "generic_pool" src/ --include="*.cpp" --include="*.hpp" --include="*.h"
```

Expected: Zero matches.

**Step 5: Verify all pools are centralized**

```bash
grep -r "etl::pool<" src/ --include="*.cpp" --include="*.hpp" --include="*.h" \
  | grep -v "kstd_pool.hpp" | grep -v "kstd_pools.hpp"
```

Expected: Zero matches (all pool instances live in kstd_pools.hpp).

**Step 6: Code style check**

```bash
pre-commit run --all-files
```

Fix any formatting issues.

**Step 7: Final commit**

```bash
git add -A
git commit --signoff -m "style(kstd): fix formatting after ETL migration"
```

---

## Appendix: ETL API Quick Reference

### etl::vector_ext<T>
```cpp
// Construction: requires external buffer
vector_ext(void* buffer, size_t max_size);
vector_ext(size_t initial_size, void* buffer, size_t max_size);

// API: same as std::vector
push_back(const T&), push_back(T&&), emplace_back(Args&&...)
pop_back(), clear(), resize(n), reserve(n)  // reserve is no-op
begin(), end(), size(), empty(), capacity(), max_size()
operator[], at(n), front(), back()
```

### etl::list_ext<T>
```cpp
// Construction: requires node pool
explicit list_ext(etl::ipool& node_pool);
list_ext(size_t initial_size, etl::ipool& node_pool);

// Pool type for creating compatible pools:
using pool_type = typename etl::ilist<T>::data_node_t;
// Usage: etl::pool<list_ext<T>::pool_type, N> my_pool;

// API: same as std::list
push_back(const T&), push_front(const T&)
pop_back(), pop_front(), clear()
begin(), end(), size(), empty()
insert(pos, val), erase(pos)
sort(), reverse(), unique()
```

### etl::ipool (used by unordered_map custom impl)
```cpp
// Allocate raw memory for one object
template <typename T> T* allocate();

// Construct object in pool memory
template <typename T, typename... Args> T* create(Args&&...);

// Destroy object and return memory to pool
template <typename T> void destroy(const T* p);

// Release without calling destructor
bool release(const void* p);

// Query
size_t size() const;       // Currently allocated
size_t available() const;  // Remaining capacity
size_t max_size() const;   // Total capacity
bool empty() const;
bool full() const;
```

---

## Open Questions / Risks

1. **Vector buffer ownership**: `etl::vector_ext<T>` does NOT own its buffer. If the buffer is stack-allocated and the vector outlives the scope, UB. For class members, all buffers come from `kstd_pools.hpp` (file-scope `inline` storage — same lifetime as the program).

2. **Max capacity**: All ETL containers have compile-time max capacity. Currently sk_std containers can grow unbounded (doubling). We need to choose reasonable max values (e.g., `kMaxTasks = 64`, `kMaxBuckets = 128`). Adjust in `kstd_pools.hpp` as needed.

3. **`reserve()` / `resize()` semantics**: `etl::vector_ext` does not support `reserve()` that grows the buffer. It's a no-op. Code that relies on `reserve()` to increase capacity needs rethinking.

4. **`std::hash` / `std::equal_to`**: The custom unordered_map uses `std::hash` and `std::equal_to`. In freestanding mode, these may not be available. May need to provide `kstd::hash` equivalents or use ETL's hash if available.

5. **`sk_iostream` / `sk_cstdio` / `sk_cstring`**: These are C/C++ runtime wrappers, NOT containers. They are NOT migrated to ETL and remain as-is.

6. **Pool sharing**: Multiple containers can share a pool (e.g., multiple `list<T*>` sharing one node pool). This is safe with `etl::ipool` but requires careful capacity planning in `kstd_pools.hpp`.
