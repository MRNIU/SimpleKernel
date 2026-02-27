# ETL Containers Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 `src/task/include/` 中所有 `kstd::static_*` 容器替换为直接使用 `etl::` 容器，并引入 `kernel_config.hpp` 集中管理容量常量。

> **前置条件：** 本计划依赖 `2026-02-27-etl-delegate-fsm-flags.md` 中 Task 1.1 已完成（`ETL_USE_ASSERT_FUNCTION` 已启用，`KernelEtlAssertHandler` 已注册）。

**Architecture:**
- `kstd::static_vector<T,N>` / `kstd::static_list<T,N>` 已是 ETL 的 `using` 别名，API 完全兼容，只需改命名空间和头文件。
- `kstd::priority_queue` 是自研适配器（持有外部容器**引用**），需替换为自包含的 `etl::priority_queue<T,N,Cmp>`，同时删除独立 storage 成员。
- `kstd::static_unordered_map` 是自研封装（内部有 pool + 外部 bucket buffer），需替换为 `etl::unordered_map<K,V,N,B>`，简化结构并避免自研节点池。
- 所有容量魔法数字统一提取到 `src/include/kernel_config.hpp`。

**Tech Stack:** C++23, ETL (3rd/etl/), etl::vector, etl::list, etl::priority_queue, etl::unordered_map

---

## 背景知识：迁移前必读

### kstd 与 ETL 的关系

| kstd 类型 | 实现方式 | 迁移复杂度 |
|---|---|---|
| `kstd::static_vector<T,N>` | `using static_vector = etl::vector<T,N>` | **极低**：只改头文件和命名空间 |
| `kstd::static_list<T,N>` | `using static_list = etl::list<T,N>` | **极低**：只改头文件和命名空间 |
| `kstd::priority_queue<T,Container,Cmp>` | 自研适配器，持有外部容器引用，内部用 `std::make_heap` | **中等**：需删除独立 storage，改成 `etl::priority_queue` 自包含 |
| `kstd::static_unordered_map<K,V,N,B>` | 自研封装，内含 `kstd::Pool<Node,N>` + `void*[B]` bucket buffer | **中等**：改成 `etl::unordered_map<K,V,N,B>` 需确认 hash 特化 |

### `etl::priority_queue` 签名

```cpp
// 头文件: etl/priority_queue.h
// 自包含版本，内部存储，无需外部 storage
etl::priority_queue<T, MaxSize, Compare>
```

与 `kstd::priority_queue` 的差异：
- **无需独立 storage 成员**：旧代码有 `sleeping_tasks_storage` 成员 + `sleeping_tasks{sleeping_tasks_storage}` 构造，新版只需一个成员。
- **满检查**：`etl::priority_queue` 提供 `full()` 方法，使用前务必检查。

### `etl::unordered_map` 签名

```cpp
// 头文件: etl/unordered_map.h
// etl::unordered_map<Key, Value, MaxSize, MaxBuckets, Hash, KeyEqual>
etl::unordered_map<ResourceId, etl::list<TaskControlBlock*, 16>, 32, 64> blocked_tasks;
```

与 `kstd::static_unordered_map` 的差异：
- ETL 需要 `etl::hash<Key>` 特化（而非 `std::hash<Key>`）。
- 对于 `Pid`（`size_t`）和 `uint64_t`，ETL 内置哈希，无需特化。
- 对于 `ResourceId`（自定义类型），需在 `etl` 命名空间提供特化。

### ETL 内部检查配置

`src/include/etl_profile.h` 已启用 `ETL_USE_ASSERT_FUNCTION 1`（由 `etl-delegate-fsm-flags.md` Task 1.1 完成），ETL 内部断言失败时将调用 `KernelEtlAssertHandler` 输出错误日志并 panic。

**此计划必须在 `etl-delegate-fsm-flags.md` Task 1.1 完成后执行。**

尽管 ETL 运行时检查已启用，每次 insert/push 前**仍应显式检查 `.full()`**，原因：
1. 提供更有意义的错误日志（说明是哪个队列满了）
2. 允许优雅降级而非直接 panic
3. 明确表达代码意图

---

## Task 1: 新建 `src/include/kernel_config.hpp`

**Files:**
- Create: `src/include/kernel_config.hpp`

**Step 1: 创建文件**

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_

#include <cstddef>

namespace kernel::config {

// ── 任务管理容量 ────────────────────────────────────────────────
/// 全局最大任务数（task_table_ 容量）
inline constexpr size_t kMaxTasks = 128;
/// task_table_ 桶数（建议 = 2 × kMaxTasks）
inline constexpr size_t kMaxTasksBuckets = 256;

/// 每个 CPU 的最大睡眠任务数（sleeping_tasks 容量）
inline constexpr size_t kMaxSleepingTasks = 64;

/// 阻塞队列：最大资源组数（blocked_tasks 的 map 容量）
inline constexpr size_t kMaxBlockedGroups = 32;
/// 阻塞队列：map 桶数
inline constexpr size_t kMaxBlockedGroupsBuckets = 64;
/// 阻塞队列：每组最大阻塞任务数（etl::list 容量）
inline constexpr size_t kMaxBlockedPerGroup = 16;

/// 调度器就绪队列容量（FIFO / RR / CFS）
inline constexpr size_t kMaxReadyTasks = 64;

// ── 中断线程映射容量 ─────────────────────────────────────────────
/// 最大中断线程数
inline constexpr size_t kMaxInterruptThreads = 32;
/// 中断线程 map 桶数
inline constexpr size_t kMaxInterruptThreadsBuckets = 64;

}  // namespace kernel::config

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_
```

**Step 2: 验证文件存在**

```bash
ls src/include/kernel_config.hpp
```
期望输出：文件路径

**Step 3: 运行格式检查**

```bash
pre-commit run --files src/include/kernel_config.hpp
```
期望：PASSED（或只有 whitespace 类提示）

**Step 4: Commit**

```bash
git add src/include/kernel_config.hpp
git commit -m "feat(task): add kernel_config.hpp for centralized capacity constants" --signoff
```

---

## Task 2: 检查 `ResourceId` 是否需要 `etl::hash` 特化

**Files:**
- Read: `src/task/include/resource_id.hpp`

**Step 1: 查看 ResourceId 定义**

```bash
cat src/task/include/resource_id.hpp
```

**Step 2: 分析**

- 如果 `ResourceId` 是整数类型或 `size_t` 的 typedef/using，ETL 内置 hash，**无需特化**，跳到 Task 3。
- 如果 `ResourceId` 是自定义类/结构体，需在 `kernel_config.hpp` 或专用头文件中添加：

```cpp
// 仅在 ResourceId 是自定义类型时添加
#include "etl/hash.h"

namespace etl {
template <>
struct hash<ResourceId> {
  size_t operator()(const ResourceId& id) const {
    return etl::hash<uint64_t>{}(static_cast<uint64_t>(id));
    // 根据 ResourceId 的实际字段调整
  }
};
}  // namespace etl
```

**Step 3: Commit（如需特化）**

```bash
git add src/include/kernel_config.hpp  # 或特化所在文件
git commit -m "feat(task): add etl::hash specialization for ResourceId" --signoff
```

---

## Task 3: 迁移 `fifo_scheduler.hpp` — static_list → etl::list

**Files:**
- Modify: `src/task/include/fifo_scheduler.hpp`

**Step 1: 阅读当前文件**

确认 L8、L94 的 `kstd_list` 和 `kstd::static_list<TaskControlBlock*, 64>` 是唯一需要改动的地方。

**Step 2: 修改内容**

将：
```cpp
// L8
#include "kstd_list"
// L94
kstd::static_list<TaskControlBlock*, 64> ready_queue;
```

改为：
```cpp
// L8
#include "etl/list.h"
#include "kernel_config.hpp"
// L94
etl::list<TaskControlBlock*, kernel::config::kMaxReadyTasks> ready_queue;
```

同时删除重复注释（原文件 L92-L93 有两行相同注释，只保留一行）。

**Step 3: 运行 LSP 检查**

检查 `fifo_scheduler.hpp` 无编译错误。

**Step 4: 格式检查**

```bash
pre-commit run --files src/task/include/fifo_scheduler.hpp
```

**Step 5: Commit**

```bash
git add src/task/include/fifo_scheduler.hpp
git commit -m "refactor(task): replace kstd::static_list with etl::list in FifoScheduler" --signoff
```

---

## Task 4: 迁移 `rr_scheduler.hpp` — static_list → etl::list

**Files:**
- Modify: `src/task/include/rr_scheduler.hpp`

**Step 1: 修改内容**

将：
```cpp
// L8
#include "kstd_list"
// L121
kstd::static_list<TaskControlBlock*, 64> ready_queue;
```

改为：
```cpp
// L8
#include "etl/list.h"
#include "kernel_config.hpp"
// L121
etl::list<TaskControlBlock*, kernel::config::kMaxReadyTasks> ready_queue;
```

同时删除重复注释（原文件 L119-L120 有两行相同注释，只保留一行）。

**Step 2: 格式检查**

```bash
pre-commit run --files src/task/include/rr_scheduler.hpp
```

**Step 3: Commit**

```bash
git add src/task/include/rr_scheduler.hpp
git commit -m "refactor(task): replace kstd::static_list with etl::list in RoundRobinScheduler" --signoff
```

---

## Task 5: 迁移 `cfs_scheduler.hpp` — static_vector + priority_queue → etl::priority_queue

这是变动最大的调度器文件，需要：
1. 删除 `ready_queue_storage_` 成员（独立 storage 向量）
2. 将 `kstd::priority_queue<T, Container, Cmp>` 替换为 `etl::priority_queue<T, N, Cmp>`
3. 删除 `Dequeue()` 中用于重建堆的临时 `kstd::static_vector<TaskControlBlock*, 64> temp`，改用 `etl::vector` 或直接使用 `etl::priority_queue` 的遍历

**Files:**
- Modify: `src/task/include/cfs_scheduler.hpp`

**Step 1: 阅读当前文件**

确认以下需要改动的位置：
- L11: `#include "kstd_vector"`
- L85: `kstd::static_vector<TaskControlBlock*, 64> temp;`（`Dequeue()` 内部局部变量）
- L209: `kstd::static_vector<TaskControlBlock*, 64> ready_queue_storage_;`（成员变量）
- L211-L214: `kstd::priority_queue<...> ready_queue_{ready_queue_storage_};`（成员变量）

**Step 2: 修改 includes**

将：
```cpp
#include "kstd_cassert"
#include "kstd_vector"
```

改为：
```cpp
#include "etl/priority_queue.h"
#include "etl/vector.h"
#include "kernel_config.hpp"
#include "kstd_cassert"
```

**Step 3: 修改 `Dequeue()` 中的临时变量（L85）**

将：
```cpp
kstd::static_vector<TaskControlBlock*, 64> temp;
```

改为：
```cpp
etl::vector<TaskControlBlock*, kernel::config::kMaxReadyTasks> temp;
```

**Step 4: 删除 `ready_queue_storage_` 成员，修改 `ready_queue_` 成员**

将 private 成员区域（L208-L214）：
```cpp
/// 就绪队列底层存储 (固定容量)
kstd::static_vector<TaskControlBlock*, 64> ready_queue_storage_;
/// 就绪队列 (优先队列，按 vruntime 排序)
kstd::priority_queue<TaskControlBlock*,
                     kstd::static_vector<TaskControlBlock*, 64>,
                     VruntimeCompare>
    ready_queue_{ready_queue_storage_};
```

改为：
```cpp
/// 就绪队列 (优先队列，按 vruntime 排序)
etl::priority_queue<TaskControlBlock*, kernel::config::kMaxReadyTasks,
                    VruntimeCompare>
    ready_queue_;
```

**Step 5: 确认调用点不变**

`ready_queue_.push(task)`、`ready_queue_.top()`、`ready_queue_.pop()`、`ready_queue_.empty()`、`ready_queue_.size()` 的接口与 `kstd::priority_queue` 一致，无需修改。

注意：`etl::priority_queue` 也提供 `full()` 方法。在 `Enqueue()` 中（L53）`ready_queue_.push(task)` 之前应加满检查（见 Task 8 中统一处理）。

**Step 6: 格式检查**

```bash
pre-commit run --files src/task/include/cfs_scheduler.hpp
```

**Step 7: Commit**

```bash
git add src/task/include/cfs_scheduler.hpp
git commit -m "refactor(task): replace kstd priority_queue+storage with etl::priority_queue in CfsScheduler" --signoff
```

---

## Task 6: 迁移 `task_manager.hpp` — CpuSchedData 中的容器

**Files:**
- Modify: `src/task/include/task_manager.hpp`

### 6.1 `CpuSchedData` 中的 sleeping_tasks

**当前（L39-L44）：**
```cpp
kstd::static_vector<TaskControlBlock*, 64> sleeping_tasks_storage;
kstd::priority_queue<TaskControlBlock*,
                     kstd::static_vector<TaskControlBlock*, 64>,
                     TaskControlBlock::WakeTickCompare>
    sleeping_tasks{sleeping_tasks_storage};
```

**替换为：**
```cpp
/// 睡眠队列 (优先队列，按唤醒时间排序)
etl::priority_queue<TaskControlBlock*, kernel::config::kMaxSleepingTasks,
                    TaskControlBlock::WakeTickCompare>
    sleeping_tasks;
```

删除 `sleeping_tasks_storage` 成员。

### 6.2 `CpuSchedData` 中的 blocked_tasks

**当前（L47-L49）：**
```cpp
kstd::static_unordered_map<ResourceId,
                           kstd::static_list<TaskControlBlock*, 16>, 32, 64>
    blocked_tasks;
```

**替换为：**
```cpp
/// 阻塞队列 (按资源 ID 分组)
etl::unordered_map<ResourceId,
                   etl::list<TaskControlBlock*, kernel::config::kMaxBlockedPerGroup>,
                   kernel::config::kMaxBlockedGroups,
                   kernel::config::kMaxBlockedGroupsBuckets>
    blocked_tasks;
```

### 6.3 `TaskManager` 私有成员中的 task_table_

**当前（L207-L208）：**
```cpp
kstd::static_unordered_map<Pid, kstd::unique_ptr<TaskControlBlock>, 64, 128>
    task_table_;
```

**替换为（注意：etl::unordered_map 的 value 需支持默认构造，`kstd::unique_ptr` 需验证）：**

> **风险点**：`etl::unordered_map` 在节点初始化时可能要求 Value 默认可构造。`kstd::unique_ptr` 若默认构造为 `nullptr`（通常是），则可行；否则改为裸指针 + pool 管理。

```cpp
/// 全局任务表 (PID -> TCB 映射)
etl::unordered_map<Pid, kstd::unique_ptr<TaskControlBlock>,
                   kernel::config::kMaxTasks,
                   kernel::config::kMaxTasksBuckets>
    task_table_;
```

**如果 unique_ptr 不兼容，改用裸指针：**
```cpp
etl::unordered_map<Pid, TaskControlBlock*,
                   kernel::config::kMaxTasks,
                   kernel::config::kMaxTasksBuckets>
    task_table_;
```
（此时 TCB 生命周期由 `etl::pool<TaskControlBlock, kMaxTasks>` 管理，见 Task 9 可选优化。）

### 6.4 interrupt_threads_ 和 interrupt_work_queues_

**当前（L213-L217）：**
```cpp
kstd::static_unordered_map<uint64_t, TaskControlBlock*, 32, 64>
    interrupt_threads_;
kstd::static_unordered_map<uint64_t, InterruptWorkQueue*, 32, 64>
    interrupt_work_queues_;
```

**替换为：**
```cpp
/// 中断号 -> 中断线程映射
etl::unordered_map<uint64_t, TaskControlBlock*,
                   kernel::config::kMaxInterruptThreads,
                   kernel::config::kMaxInterruptThreadsBuckets>
    interrupt_threads_;
/// 中断号 -> 工作队列映射
etl::unordered_map<uint64_t, InterruptWorkQueue*,
                   kernel::config::kMaxInterruptThreads,
                   kernel::config::kMaxInterruptThreadsBuckets>
    interrupt_work_queues_;
```

### 6.5 `GetThreadGroup` 返回类型（L245）

**当前：**
```cpp
kstd::static_vector<TaskControlBlock*, 64> GetThreadGroup(Pid tgid);
```

**替换为：**
```cpp
etl::vector<TaskControlBlock*, kernel::config::kMaxReadyTasks> GetThreadGroup(Pid tgid);
```

### 6.6 修改 includes（L17-L21）

将：
```cpp
#include "kstd_list"
#include "kstd_priority_queue"
#include "kstd_unique_ptr"
#include "kstd_unordered_map"
#include "kstd_vector"
```

改为：
```cpp
#include "etl/list.h"
#include "etl/priority_queue.h"
#include "etl/unordered_map.h"
#include "etl/vector.h"
#include "kernel_config.hpp"
#include "kstd_unique_ptr"
```

**Step 2: 格式检查**

```bash
pre-commit run --files src/task/include/task_manager.hpp
```

**Step 3: Commit**

```bash
git add src/task/include/task_manager.hpp
git commit -m "refactor(task): replace kstd containers with etl:: in TaskManager and CpuSchedData" --signoff
```

---

## Task 7: 更新 `task_manager.cpp` — GetThreadGroup 返回类型

**Files:**
- Modify: `src/task/task_manager.cpp`

**Step 1: 阅读 GetThreadGroup 实现**

```bash
grep -n "GetThreadGroup\|static_vector\|kstd::" src/task/task_manager.cpp
```

**Step 2: 修改返回类型和局部变量**

找到类似以下代码（约 L195-L200）：
```cpp
kstd::static_vector<TaskControlBlock*, 64> TaskManager::GetThreadGroup(Pid tgid) {
  kstd::static_vector<TaskControlBlock*, 64> result;
  // ...
  return result;
}
```

修改为：
```cpp
etl::vector<TaskControlBlock*, kernel::config::kMaxReadyTasks>
TaskManager::GetThreadGroup(Pid tgid) {
  etl::vector<TaskControlBlock*, kernel::config::kMaxReadyTasks> result;
  // ...
  return result;
}
```

**Step 3: 修改 includes**

在 `task_manager.cpp` 顶部，将 `kstd_vector` 相关 include 替换为 `etl/vector.h` 和 `kernel_config.hpp`。

**Step 4: 运行格式和 LSP 检查**

```bash
pre-commit run --files src/task/task_manager.cpp
```

**Step 5: Commit**

```bash
git add src/task/task_manager.cpp
git commit -m "refactor(task): update GetThreadGroup return type to etl::vector" --signoff
```

---

## Task 8: 审计所有 push/insert 调用点，添加 `.full()` 检查

`ETL_USE_ASSERT_FUNCTION=1` 下，容器满时断言失败将触发 `KernelEtlAssertHandler`。仍需显式审计所有插入点，确保优雅降级。

**Files:**
- Read/Modify: `src/task/task_manager.cpp`, 各调度器 .hpp

**Step 1: 找出所有插入点**

```bash
grep -rn "push\|insert\|push_back\|emplace" src/task/ --include="*.cpp" --include="*.hpp"
```

**Step 2: 对每个插入点，检查是否有前置 `.full()` 检查**

模式如下——以 `sleeping_tasks.push(tcb)` 为例：

**修改前（无检查，不安全）：**
```cpp
sleeping_tasks.push(tcb);
```

**修改后（安全）：**
```cpp
if (!sched_data.sleeping_tasks.full()) {
  sched_data.sleeping_tasks.push(tcb);
} else {
  klog::Err() << "sleeping_tasks full, cannot sleep task: " << tcb->name;
  // 根据调用上下文决定是否 panic 或返回错误
}
```

对 `blocked_tasks`、`task_table_`、`interrupt_threads_`、`interrupt_work_queues_` 的 insert 同理。

**Step 3: 对调度器 Enqueue 方法添加检查**

例如 `FifoScheduler::Enqueue()`：
```cpp
void Enqueue(TaskControlBlock* task) override {
  if (!ready_queue.full()) {
    ready_queue.push_back(task);
    stats_.total_enqueues++;
  } else {
    klog::Err() << "FifoScheduler: ready_queue full, dropping task";
  }
}
```

**Step 4: Commit**

```bash
git add src/task/
git commit -m "fix(task): add .full() checks before all etl container insertions" --signoff
```

---

## Task 9: 构建验证

**Step 1: 构建 x86_64（最快）**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel 2>&1 | tail -20
```

期望：`make[2]: 'SimpleKernel' is up to date` 或正常链接成功。

**Step 2: 运行单元测试**

```bash
cd build_x86_64 && make unit-test 2>&1 | tail -30
```

期望：所有测试通过（PASSED）。

**Step 3: 如有编译错误**

常见问题及解决方法：

| 错误 | 原因 | 解决 |
|---|---|---|
| `etl::hash<ResourceId> not defined` | ResourceId 缺少 hash 特化 | 回到 Task 2，添加 `etl::hash` 特化 |
| `kstd::unique_ptr is not default constructible` | ETL map value 类型约束 | 将 task_table_ value 改为裸指针 |
| `etl::priority_queue::push undefined` | 头文件未包含 | 确认 `#include "etl/priority_queue.h"` |
| `no match for operator=` on priority_queue | 旧代码引用了 storage | 确认已删除 `sleeping_tasks_storage` |

**Step 4: 构建另一架构（可选验证）**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

---

## Task 10: 清理遗留 kstd 头文件引用

在所有迁移完成、构建通过后，检查是否还有遗留引用：

**Step 1: 检查是否还有遗留 kstd 容器引用**

```bash
grep -rn "kstd_list\|kstd_vector\|kstd_unordered_map\|kstd_priority_queue\|kstd::static_" src/task/ --include="*.cpp" --include="*.hpp"
```

期望：无输出（或只剩确实需要保留的）。

**Step 2: 检查 kstd_vector/kstd_list 头文件本身是否还被其他模块引用**

```bash
grep -rn "kstd_list\|kstd_vector\|kstd_unordered_map\|kstd_priority_queue" src/ --include="*.cpp" --include="*.hpp" | grep -v "^src/libcxx/"
```

如果其他模块（如 `src/device/`）也在用，暂不删除这些头文件，仅完成 task 模块的迁移。

**Step 3: Final Commit**

```bash
git add -A
git commit -m "refactor(task): complete etl container migration, clean up kstd includes" --signoff
```

---

## 迁移速查表（执行参考）

| 当前代码 | 替换为 | 注意 |
|---|---|---|
| `#include "kstd_list"` | `#include "etl/list.h"` + `#include "kernel_config.hpp"` | |
| `#include "kstd_vector"` | `#include "etl/vector.h"` + `#include "kernel_config.hpp"` | |
| `#include "kstd_priority_queue"` | `#include "etl/priority_queue.h"` | |
| `#include "kstd_unordered_map"` | `#include "etl/unordered_map.h"` | 需 etl::hash |
| `kstd::static_list<T, N>` | `etl::list<T, kernel::config::kConst>` | |
| `kstd::static_vector<T, N>` | `etl::vector<T, kernel::config::kConst>` | |
| `kstd::static_unordered_map<K,V,N,B>` | `etl::unordered_map<K,V,kN,kB>` | 需 etl::hash<K> |
| `storage_` 成员 + `kstd::priority_queue<T,Container,Cmp> q_{storage_}` | `etl::priority_queue<T,kN,Cmp> q_` | 删除独立 storage |

---

## 风险与回滚

- **所有改动都在头文件中**，不涉及算法逻辑变更，风险可控。
- 如遇到 `etl::unordered_map` 与 `kstd::unique_ptr` 兼容性问题，可暂时保留 `task_table_` 的 `kstd::static_unordered_map`，其余全量迁移。
- 任何步骤失败，`git revert <commit>` 即可回滚单个 task。
