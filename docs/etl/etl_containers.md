# ETL 固定容量容器在 SimpleKernel 中的应用

> 适用版本：ETL 已集成于 `3rd/etl/`，内核配置见 `src/include/etl_profile.h`

---

## 目录

1. [为什么需要固定容量容器](#1-为什么需要固定容量容器)
2. [ETL 容器体系总览](#2-etl-容器体系总览)
3. [etl::vector — 替代 kstd::static_vector](#3-etlvector--替代-kstdstatic_vector)
4. [etl::list — 替代 kstd::static_list](#4-etllist--替代-kstdstatic_list)
5. [etl::unordered_map — 替代 kstd::static_unordered_map](#5-etlunordered_map--替代-kstdstatic_unordered_map)
6. [etl::pool — TCB 内存池分配](#6-etlpool--tcb-内存池分配)
7. [etl::variant — 替代 std::variant](#7-etlvariant--替代-stdvariant)
8. [ivector / ilist / iunordered_map — 接口基类](#8-ivector--ilist--iunordered_map--接口基类)
9. [容量满时的行为与 ETL_NO_CHECKS](#9-容量满时的行为与-etl_no_checks)
10. [迁移速查表](#10-迁移速查表)
11. [使用限制汇总](#11-使用限制汇总)

---

## 1. 为什么需要固定容量容器

SimpleKernel 是裸机内核，**禁止使用堆分配**（内存子系统初始化之前调用 `new` 会崩溃），因此：

- `std::vector`、`std::list`、`std::unordered_map` 均不可用（内部调用 `new`）
- 项目自研了 `kstd::static_vector<T, N>`、`kstd::static_list<T, N>`、`kstd::static_unordered_map<K, V, N, BucketN>` 作为替代

ETL 提供的固定容量容器与 `kstd::*` 高度对应，额外的优势是：
- 更完整的 STL 兼容 API（`emplace_back`、`insert`、`erase` 等）
- `ivector<T>` / `ilist<T>` 接口基类，让函数参数与容量解耦
- `etl::pool<T, N>` 提供对象池，用于 TCB 等大量同类型对象的分配
- `etl::variant` 在 C++11/14 项目中替代 `std::variant`（本项目已用 C++23，保留 `std::variant` 亦可）

---

## 2. ETL 容器体系总览

```
etl::vector<T, N>      ←→  kstd::static_vector<T, N>
etl::list<T, N>        ←→  kstd::static_list<T, N>
etl::unordered_map<K, V, N, H, KP, Bucket>  ←→  kstd::static_unordered_map<K, V, N, BucketN>
etl::flat_map<K, V, N> — 有序数组实现，无哈希开销，适合小型映射
etl::pool<T, N>        — 对象池，无对应 kstd 版本
etl::variant<T...>     ←→  std::variant<T...>（C++23 可直接用 std::variant）

接口基类（类型擦除容量参数）：
  etl::ivector<T>      — 接受任意 N 的 etl::vector<T, N>
  etl::ilist<T>        — 接受任意 N 的 etl::list<T, N>
  etl::iunordered_map<K,V> — 接受任意 N 的 etl::unordered_map
```

---

## 3. etl::vector — 替代 kstd::static_vector

### 3.1 头文件与类型

```cpp
#include <etl/vector.h>

// 声明：etl::vector<元素类型, 最大容量>
etl::vector<TaskControlBlock*, 64> sleeping_tasks_storage;

// 接口基类（容量无关）
etl::ivector<TaskControlBlock*>& ref = sleeping_tasks_storage;
```

### 3.2 当前项目中的使用位置

```cpp
// src/task/include/task_manager.hpp — CpuSchedData
kstd::static_vector<TaskControlBlock*, 64> sleeping_tasks_storage;
//      ↑ 可替换为：
etl::vector<TaskControlBlock*, 64> sleeping_tasks_storage;

// 返回值（task_manager.hpp 私有方法）
kstd::static_vector<TaskControlBlock*, 64> GetThreadGroup(Pid tgid);
//      ↑ 可替换为：
etl::vector<TaskControlBlock*, 64> GetThreadGroup(Pid tgid);
// 或用接口基类（函数签名与容量解耦）：
void ProcessThreadGroup(etl::ivector<TaskControlBlock*>& group);
```

### 3.3 核心 API

```cpp
etl::vector<int, 16> v;

v.push_back(42);             // 追加元素（若满且 ETL_NO_CHECKS=1，行为未定义）
v.emplace_back(42);          // 原地构造追加
v.pop_back();                // 删除末尾元素
v.insert(v.begin() + 1, 7); // 在位置 1 插入
v.erase(v.begin());          // 删除第一个元素
v.clear();                   // 清空

v.size();                    // 当前元素数
v.capacity();                // == N，编译期常量
v.full();                    // 是否已满（容量检查的关键！）
v.empty();                   // 是否为空
v[0];                        // 下标访问（无边界检查，ETL_NO_CHECKS=1 时）
v.front(); v.back();         // 首尾元素
v.data();                    // 原始指针（T*），与 C API 互操作

// 范围 for 循环
for (auto* tcb : v) { ... }
```

### 3.4 与 kstd::priority_queue 配合

`task_manager.hpp` 中的睡眠队列是以 `kstd::static_vector` 为底层存储的优先队列：

```cpp
// 当前代码
kstd::priority_queue<TaskControlBlock*,
                     kstd::static_vector<TaskControlBlock*, 64>,
                     TaskControlBlock::WakeTickCompare>
    sleeping_tasks{sleeping_tasks_storage};
```

ETL 自带 `etl::priority_queue<T, N, TCompare>`，内部使用固定数组，可直接替换：

```cpp
#include <etl/priority_queue.h>

etl::priority_queue<TaskControlBlock*, 64, TaskControlBlock::WakeTickCompare>
    sleeping_tasks;

// push / pop / top 接口与 std::priority_queue 一致
sleeping_tasks.push(tcb);
TaskControlBlock* next = sleeping_tasks.top();
sleeping_tasks.pop();
sleeping_tasks.full();   // 满检查
```

---

## 4. etl::list — 替代 kstd::static_list

### 4.1 头文件与类型

```cpp
#include <etl/list.h>

etl::list<TaskControlBlock*, 16> blocked_list;
```

### 4.2 当前项目中的使用位置

```cpp
// src/task/include/task_manager.hpp — 阻塞队列
kstd::static_unordered_map<ResourceId,
                           kstd::static_list<TaskControlBlock*, 16>, 32, 64>
    blocked_tasks;
// ↑ kstd::static_list<TaskControlBlock*, 16> 可替换为 etl::list<TaskControlBlock*, 16>
```

### 4.3 核心 API

```cpp
etl::list<TaskControlBlock*, 16> lst;

lst.push_back(tcb);          // 尾部追加
lst.push_front(tcb);         // 头部插入
lst.pop_back();               // 删除尾部
lst.pop_front();              // 删除头部
lst.remove(tcb);              // 删除所有匹配元素（线性扫描）
lst.erase(it);                // 按迭代器删除（O(1)）

lst.size();                   // 当前元素数
lst.full();                   // 是否已满
lst.empty();                  // 是否为空
lst.front(); lst.back();      // 首尾元素

// 遍历
for (auto* tcb : lst) { ... }

// 常见模式：遍历并条件删除
for (auto it = lst.begin(); it != lst.end(); ) {
  if ((*it)->status == TaskStatus::kExited) {
    it = lst.erase(it);
  } else {
    ++it;
  }
}
```

### 4.4 与 kstd::static_list 的区别

| 特性 | kstd::static_list | etl::list |
|---|---|---|
| 内存布局 | 侵入式 or 数组节点池 | 内部节点池（`pool<node, N>`）|
| `remove(value)` | 可能无 | ✅ 有 |
| `erase(iterator)` | ✅ | ✅ |
| 接口基类 | ❌ | `etl::ilist<T>` ✅ |
| `full()` 检查 | 取决于实现 | ✅ 有 |

---

## 5. etl::unordered_map — 替代 kstd::static_unordered_map

### 5.1 头文件与类型

```cpp
#include <etl/unordered_map.h>

// etl::unordered_map<Key, Value, MaxSize, MaxBuckets, Hash, KeyEqual>
// MaxBuckets 默认为 MaxSize（自动推导），建议显式设为 MaxSize 的 2 倍（减少冲突）
etl::unordered_map<ResourceId, etl::list<TaskControlBlock*, 16>, 32, 64> blocked_tasks;
```

### 5.2 当前项目中的使用位置

```cpp
// CpuSchedData：阻塞队列（按资源 ID 分组）
kstd::static_unordered_map<ResourceId,
                           kstd::static_list<TaskControlBlock*, 16>, 32, 64>
    blocked_tasks;
// ↑ 可替换为：
etl::unordered_map<ResourceId,
                   etl::list<TaskControlBlock*, 16>, 32, 64>
    blocked_tasks;

// TaskManager 私有：全局任务表（PID -> TCB 映射）
kstd::static_unordered_map<Pid, kstd::unique_ptr<TaskControlBlock>, 64, 128>
    task_table_;
// ↑ 可替换为（注意：etl::unordered_map 的 value 需支持默认构造）：
etl::unordered_map<Pid, TaskControlBlock*, 64, 128> task_table_;
// （若使用 unique_ptr，需确认 ETL unordered_map 对 move-only 类型的支持）

// 中断映射
kstd::static_unordered_map<uint64_t, TaskControlBlock*, 32, 64>
    interrupt_threads_;
// ↑ 可替换为：
etl::unordered_map<uint64_t, TaskControlBlock*, 32, 64> interrupt_threads_;
```

### 5.3 核心 API

```cpp
etl::unordered_map<uint64_t, TaskControlBlock*, 32, 64> map;

// 插入
map.insert({42UL, tcb});
map[42] = tcb;               // 若键不存在且未满，则插入；若满且 ETL_NO_CHECKS=1，UB！

// 查找
auto it = map.find(42UL);
if (it != map.end()) {
  TaskControlBlock* tcb = it->second;
}

// 删除
map.erase(42UL);             // 按键删除
map.erase(it);               // 按迭代器删除

// 状态检查
map.size();
map.full();                  // 务必在 insert/operator[] 前检查！
map.contains(key);           // C++20-style，ETL 也提供此 API

// 遍历
for (auto& [key, val] : map) { ... }
```

### 5.4 etl::flat_map — 小型有序映射的替代方案

对于**插入后很少变动、查找频繁**的小型映射（如 `interrupt_threads_`），`etl::flat_map` 是更好的选择：

```cpp
#include <etl/flat_map.h>

// 底层是有序数组，二分查找，无哈希开销，缓存友好
etl::flat_map<uint64_t, TaskControlBlock*, 32> interrupt_threads_;

// API 与 unordered_map 基本一致，但 insert 是有序插入 O(N)，find 是 O(log N)
```

**选择建议：**

| 场景 | 推荐容器 |
|---|---|
| 频繁动态增删，key 无序 | `etl::unordered_map` |
| 初始化后不变，key 需排序或范围查询 | `etl::flat_map` |
| 超小型（<8 项） | `etl::flat_map`（cache 友好，无哈希）|

---

## 6. etl::pool — TCB 内存池分配

### 6.1 什么是 etl::pool

`etl::pool<T, N>` 是一个**对象池**，预分配 N 个 `sizeof(T)` 大小的对齐内存槽：

- 分配：`T* obj = pool.allocate<T>()` — O(1)，无堆操作
- 回收：`pool.release(obj)` — O(1)
- 内部维护空闲链表，内存完全静态

### 6.2 在 SimpleKernel 中的用途

当前 `task_table_` 使用 `kstd::unique_ptr<TaskControlBlock>` 管理 TCB 生命周期，而 `TaskControlBlock` 的构造需要分配内核栈（`uint8_t* kernel_stack`）。

可以用 `etl::pool` 管理 TCB 对象本身（不含内核栈）：

```cpp
#include <etl/pool.h>

// 预分配最多 128 个 TCB 对象
static etl::pool<TaskControlBlock, 128> tcb_pool;

// 分配 TCB（不调用构造函数，需手动 placement new）
TaskControlBlock* AllocateTcb(const char* name, int priority,
                              ThreadEntry entry, void* arg) {
  TaskControlBlock* slot = tcb_pool.allocate<TaskControlBlock>();
  if (slot == nullptr) return nullptr;  // 池满
  // placement new 调用构造函数
  return new (slot) TaskControlBlock(name, priority, entry, arg);
}

// 回收 TCB（需手动调用析构函数）
void FreeTcb(TaskControlBlock* tcb) {
  tcb->~TaskControlBlock();
  tcb_pool.release(tcb);
}
```

### 6.3 注意事项

- `etl::pool` **不调用构造函数**，需要 placement new
- `release()` **不调用析构函数**，需要手动调用
- 内核中若有虚析构函数（TCB 当前没有），需确保析构正确执行
- 适合生命周期明确、由 TaskManager 统一管理的对象

---

## 7. etl::variant — 替代 std::variant

### 7.1 现状分析

`src/device/include/driver_registry.hpp` 中：

```cpp
#include <variant>

using MatchEntry = std::variant<PlatformCompatible, PciMatchKey, AcpiHid>;
```

项目使用 C++23，`std::variant` 在 freestanding 环境下的支持取决于工具链。若遇到链接错误或 ABI 问题，可替换为 `etl::variant`。

### 7.2 etl::variant 的特点

ETL 提供两套 variant 实现：

| 版本 | 头文件 | 适用场景 |
|---|---|---|
| `etl::variant` (variadic, C++11+) | `etl/variant.h` | 类 `std::variant` API |
| `etl::legacy::variant` (C++03) | `etl/variant.h` | 旧编译器兼容 |

C++23 项目应使用 `etl::variant`（variadic 版本）。

### 7.3 使用示例

```cpp
#include <etl/variant.h>

// 替换前
using MatchEntry = std::variant<PlatformCompatible, PciMatchKey, AcpiHid>;

// 替换后（API 基本一致）
using MatchEntry = etl::variant<PlatformCompatible, PciMatchKey, AcpiHid>;

// 访问方式（etl::visit 替代 std::visit）
etl::visit([&resource](const auto& key) -> bool {
  using T = etl::decay_t<decltype(key)>;
  if constexpr (etl::is_same_v<T, PlatformCompatible>) {
    // ...
  }
  return false;
}, entry);

// holds_alternative / get
if (etl::holds_alternative<PlatformCompatible>(entry)) {
  auto& plat = etl::get<PlatformCompatible>(entry);
}
```

### 7.4 替换建议

**当前项目（C++23）无需替换 `std::variant`**：

- `std::variant` 是 C++17 标准，C++23 完全支持
- `std::visit` 配合 if constexpr 是本项目已有的惯用写法（见 `driver_registry.hpp` L131-L158）
- 替换会引入 `etl::visit` 语法差异，增加维护成本

**何时应替换：**
- 工具链不支持 `std::variant`（如某些裸机 GCC 版本）
- 需要利用 ETL variant 的额外特性（如 `etl::variant_pool`）
- 移植到 C++11/14 环境

---

## 8. ivector / ilist / iunordered_map — 接口基类

### 8.1 问题：容量参数污染函数签名

如果函数参数直接使用 `etl::vector<T, N>`，调用方必须使用完全相同的 N：

```cpp
// 糟糕的设计：容量 64 泄漏到函数签名
void ProcessTasks(etl::vector<TaskControlBlock*, 64>& tasks);

// 调用方必须用 N=64 的 vector
etl::vector<TaskControlBlock*, 32> small_list;
ProcessTasks(small_list);  // 编译错误！
```

### 8.2 解决方案：使用接口基类

ETL 为每种容器提供了**不含容量参数的接口基类**：

```cpp
// ivector<T> — 接受任意容量的 etl::vector<T, N>
void ProcessTasks(etl::ivector<TaskControlBlock*>& tasks) {
  for (auto* tcb : tasks) {
    // ...
  }
  tasks.push_back(new_tcb);  // 注意：若满则需事先检查 tasks.full()
}

// 调用方可传任意容量的 vector
etl::vector<TaskControlBlock*, 64> big_list;
etl::vector<TaskControlBlock*, 8>  small_list;
ProcessTasks(big_list);   // ✅
ProcessTasks(small_list); // ✅
```

### 8.3 应用于 task_manager.hpp

```cpp
// 替换前（私有方法签名泄漏容量）
kstd::static_vector<TaskControlBlock*, 64> GetThreadGroup(Pid tgid);

// 替换后（更灵活的返回方式：输出参数 + 接口基类）
void GetThreadGroup(Pid tgid, etl::ivector<TaskControlBlock*>& out);

// 或保留返回值，使用具体类型（内部实现细节，可接受）
etl::vector<TaskControlBlock*, 64> GetThreadGroup(Pid tgid);
```

### 8.4 接口基类速查

| ETL 接口基类 | 对应具体类型 |
|---|---|
| `etl::ivector<T>` | `etl::vector<T, N>` |
| `etl::ilist<T>` | `etl::list<T, N>` |
| `etl::iunordered_map<K,V>` | `etl::unordered_map<K,V,N,B>` |
| `etl::iflat_map<K,V>` | `etl::flat_map<K,V,N>` |
| `etl::iqueue<T>` | `etl::queue<T,N>` |
| `etl::istack<T>` | `etl::stack<T,N>` |
| `etl::ipriority_queue<T>` | `etl::priority_queue<T,N,Compare>` |

---

## 9. 容量满时的行为与 ETL_NO_CHECKS

### 9.1 当前配置

```cpp
// src/include/etl_profile.h
#define ETL_NO_CHECKS 1  // 禁用所有 ETL 内部边界检查
```

`ETL_NO_CHECKS = 1` 意味着：
- 当容器满时 `push_back`/`insert`/`operator[]` **不会抛出异常或调用错误处理器**
- 会**直接写入越界内存**，导致栈/全局变量损坏
- 这与 SimpleKernel "调用方负责前置条件" 的设计契约一致（参考 `AGENTS.md`）

### 9.2 正确的防护模式

```cpp
// 永远在插入前检查
if (!v.full()) {
  v.push_back(tcb);
} else {
  // 处理满的情况：日志 + 跳过，或触发内核 panic
  klog::Err() << "sleeping_tasks_storage full, dropping task " << tcb->name;
}

// unordered_map 插入前检查
if (!map.full()) {
  map.insert({pid, tcb});
} else {
  return std::unexpected(Error(ErrorCode::kOutOfMemory));
}
```

### 9.3 开发阶段启用检查

在调试构建中临时启用检查（修改 `etl_profile.h` 或 CMake 定义）：

```cpp
// 开发/调试时
#undef ETL_NO_CHECKS
// ETL 默认使用 assert() 检查边界，配合 GDB 可快速定位问题
```

---

## 10. 迁移速查表

| 当前类型 | ETL 替代 | 头文件 | 注意事项 |
|---|---|---|---|
| `kstd::static_vector<T, N>` | `etl::vector<T, N>` | `etl/vector.h` | API 基本兼容，接口基类 `ivector<T>` |
| `kstd::static_list<T, N>` | `etl::list<T, N>` | `etl/list.h` | 注意 `remove()` vs `erase()` 语义 |
| `kstd::static_unordered_map<K,V,N,B>` | `etl::unordered_map<K,V,N,B>` | `etl/unordered_map.h` | 桶数参数位置相同，需提供 `etl::hash<K>` |
| `kstd::priority_queue<T, Container, Cmp>` | `etl::priority_queue<T, N, Cmp>` | `etl/priority_queue.h` | 不再需要独立的 storage 对象 |
| `std::variant<T...>` | `etl::variant<T...>` | `etl/variant.h` | C++23 项目可保留 `std::variant` |
| 无对应 | `etl::pool<T, N>` | `etl/pool.h` | 对象池，需手动 placement new 和析构 |
| 无对应 | `etl::flat_map<K, V, N>` | `etl/flat_map.h` | 有序数组，适合小型不变映射 |

---

## 11. 使用限制汇总

### 11.1 与 kstd::unique_ptr 的兼容性

`etl::unordered_map` 的 value 类型需要支持**默认构造**（内部节点初始化）。
`kstd::unique_ptr<TaskControlBlock>` 若满足此条件则可用；否则需将 `task_table_` 的 value 类型改为裸指针，由 `etl::pool<TaskControlBlock>` 管理生命周期。

### 11.2 哈希函数要求

ETL `unordered_map` 需要 `etl::hash<Key>` 特化。对于 `ResourceId`、`Pid`（`size_t`）等整数类型，ETL 自带哈希支持；对于自定义类型需提供特化：

```cpp
namespace etl {
  template <>
  struct hash<ResourceId> {
    size_t operator()(const ResourceId& id) const {
      return etl::hash<uint64_t>{}(id.value());  // 根据实际字段调整
    }
  };
}
```

### 11.3 不可用的 ETL 容器

| 容器 | 原因 |
|---|---|
| `etl::string<N>` | 内核使用 `const char*` 字面量，动态字符串无需求 |
| `etl::deque<T, N>` | 内核当前无双端队列需求 |
| `etl::map<K, V, N>` | 红黑树实现，`etl::flat_map` 足够内核规模 |
| `etl::set<T, N>` | 同上 |

### 11.4 freestanding 兼容性

ETL 在 freestanding 环境下工作良好，`etl_profile.h` 已正确配置：

```cpp
#define ETL_CPP23_SUPPORTED 1
#define ETL_NO_CHECKS 1
// ETL 内部不使用动态内存分配，freestanding 环境安全
```

> **注意**：`std::allocator` 在 freestanding 环境中技术上可用（头文件存在），但 SimpleKernel **不使用**它：
> ETL 的固定容量容器本身不依赖 `std::allocator`（ETL 内部使用内联存储或节点池）。
---

## 12. 容量统一管理

各容器的容量应集中定义在一个配置头文件中，避免魔法数字散落在不同文件。建议在 `src/include/kernel_config.hpp`（或类似文件）中集中定义：

```cpp
// src/include/kernel_config.hpp
#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_

namespace kernel::config {

// 任务管理容量
inline constexpr size_t kMaxTasks          = 128;  ///< 最大同时任务数
inline constexpr size_t kMaxSleepingTasks  = 64;   ///< 最大睡眠任务数
inline constexpr size_t kMaxBlockedGroups  = 32;   ///< 最大阀塞资源组数
inline constexpr size_t kMaxBlockedPerGroup = 16;  ///< 每组最大阀塞任务数
inline constexpr size_t kMaxDrivers        = 64;   ///< 最大驱动数

}  // namespace kernel::config

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_
```

使用时引用常量：

```cpp
#include "kernel_config.hpp"
using namespace kernel::config;

// task_manager.hpp 中
etl::vector<TaskControlBlock*, kMaxSleepingTasks> sleeping_tasks_storage;
etl::unordered_map<ResourceId, etl::list<TaskControlBlock*, kMaxBlockedPerGroup>,
                   kMaxBlockedGroups, kMaxBlockedGroups>  blocked_tasks;
etl::unordered_map<Pid, kstd::unique_ptr<TaskControlBlock>,
                   kMaxTasks, kMaxTasks * 2>  task_table_;
```

这样调整系统容量时只需修改一个文件。
