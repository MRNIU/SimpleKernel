# ETL delegate、fsm、flags 在 SimpleKernel 中的应用

> 适用版本：ETL 已集成于 `3rd/etl/`，内核配置见 `src/include/etl_profile.h`

---

## 目录

1. [etl::delegate — 类型安全的函数回调](#1-etldelegate--类型安全的函数回调)
2. [etl::fsm — 任务状态机形式化](#2-etlfsm--任务状态机形式化)
3. [etl::flags / etl::bitset — 类型安全位标志](#3-etlflags--etlbitset--类型安全位标志)
4. [组合示例：delegate + flags 重构中断注册](#4-组合示例delegate--flags-重构中断注册)
5. [不建议使用的 ETL 功能](#5-不建议使用的-etl-功能)

---

## 1. etl::delegate — 类型安全的函数回调

### 1.1 问题：当前的裸函数指针

`src/include/interrupt_base.h` 中定义了中断处理函数的类型：

```cpp
// 当前：裸函数指针 typedef
typedef uint64_t (*InterruptFunc)(uint64_t cause, cpu_io::TrapContext* context);

virtual void RegisterInterruptFunc(uint64_t cause, InterruptFunc func) = 0;
virtual auto RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                       uint32_t priority,
                                       InterruptFunc handler) -> Expected<void> = 0;
```

裸函数指针的限制：
- **只能绑定自由函数或静态成员函数**，无法绑定实例方法
- 若需要绑定对象的成员函数，必须用全局变量保存对象指针，破坏封装
- 无法携带上下文（闭包），需要额外的 `void* userdata` 参数
- 类型系统无法区分"未设置"和"设置为 nullptr"

### 1.2 etl::delegate 简介

`etl::delegate<Signature>` 是 ETL 实现的**零堆分配函数委托**，类似 `std::function` 但无动态内存分配：

| 特性 | 裸函数指针 | `std::function` | `etl::delegate` |
|---|---|---|---|
| 自由函数 | ✅ | ✅ | ✅ |
| 静态成员函数 | ✅ | ✅ | ✅ |
| 实例成员函数 | ❌ | ✅ | ✅ |
| Lambda / Functor | ❌ | ✅ | ✅（需外部存储对象）|
| 堆分配 | 无 | 有 | **无** |
| RTTI 依赖 | 无 | 有 | **无** |
| sizeof | 8 | 32~48 | **16（两个指针）** |

### 1.3 核心 API

```cpp
#include <etl/delegate.h>

// 声明 delegate 类型（函数签名）
using InterruptDelegate = etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)>;

// --- 方式 1：绑定自由函数（编译期，零开销）---
uint64_t HandleTimer(uint64_t cause, cpu_io::TrapContext* ctx) { return 0; }

auto d1 = InterruptDelegate::create<HandleTimer>();

// --- 方式 2：绑定实例成员函数（运行时，持有对象引用）---
class TimerDriver {
 public:
  uint64_t OnTimer(uint64_t cause, cpu_io::TrapContext* ctx) {
    ++tick_count_;
    return 0;
  }
 private:
  uint64_t tick_count_ = 0;
};

TimerDriver timer;
// 编译期绑定：方法类型作为模板参数，对象作为模板参数
auto d2 = InterruptDelegate::create<TimerDriver, &TimerDriver::OnTimer>(timer);
// 运行期绑定（方法固定但对象在运行时确定）
auto d3 = InterruptDelegate::create<TimerDriver, &TimerDriver::OnTimer>(timer);

// --- 方式 3：检查是否已设置 ---
InterruptDelegate d_empty;      // 默认构造：未设置
bool valid = d_empty.is_valid(); // false
if (d2.is_valid()) {
  uint64_t result = d2(cause, ctx);  // 调用
}
```

### 1.4 在 interrupt_base.h 中的应用

**决策：方案 A——全面替换为 delegate，不保留裸指针接口**

将 `InterruptFunc` 裸指针替换为 `etl::delegate`，修改接口并更新所有实现：

```cpp
// interrupt_base.h —— 全面替换为 delegate
#include <etl/delegate.h>

class InterruptBase {
 public:
  // 删除裸指针 typedef，统一使用 delegate
  using InterruptDelegate = etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)>;

  virtual void RegisterInterruptFunc(uint64_t cause, InterruptDelegate delegate) = 0;
  virtual auto RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                         uint32_t priority,
                                         InterruptDelegate handler) -> Expected<void> = 0;
};
```

驱动中使用成员函数绑定，无需全局静态指针：
**方案 B：驱动内部使用 delegate（推荐，不改接口）**

> **注意**：以下示例使用 `SomeDriver` 作为说明性示例（对应内核中 `src/arch/{arch}/interrupt.cpp`
> 里的中断处理函数注册逻辑）。`Ns16550aDriver` 位于 `3rd/`，**不是内核代码**，不是迁移目标。

驱动内部将成员函数封装为 delegate，再用 lambda 包装成裸指针：

```cpp
// 迁移前：静态适配器模式（对应 src/arch/{arch}/interrupt.cpp 中的现状）
class SomeDriver {
 public:
  auto Probe(DeviceNode& node) -> Expected<void> {
    // 注册中断，将成员函数包装为裸指针
    auto& irq_ctrl = InterruptSubsystem::Get();
    irq_ctrl.RegisterInterruptFunc(irq_, &SomeDriver::StaticIrqHandler);
    return {};
  }

 private:
  // 静态适配器：将 this 指针存储在全局，再转发给成员函数
  // 这正是 delegate 要解决的问题——用 delegate 更干净
  static uint64_t StaticIrqHandler(uint64_t cause, cpu_io::TrapContext* ctx) {
    return instance_->IrqHandler(cause, ctx);  // instance_ 是全局指针
  }

  uint64_t IrqHandler(uint64_t cause, cpu_io::TrapContext* ctx) {
    // 处理中断，可访问 this
    return 0;
  }

  static SomeDriver* instance_;  // 必须全局存储 this
  uint32_t irq_ = 0;
};
```

```cpp
// 迁移后：使用 delegate 绑定成员函数（对应 src/arch/{arch}/interrupt.cpp 的迁移目标）
class SomeDriver {
 public:
  auto Probe(DeviceNode& node) -> Expected<void> {
    handler_ = etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)>
                 ::create<SomeDriver, &SomeDriver::IrqHandler>(*this);
    auto& irq_ctrl = InterruptSubsystem::Get();
    irq_ctrl.RegisterInterruptFunc(irq_, handler_);  // 直接传入 delegate
    return {};
  }

 private:
  uint64_t IrqHandler(uint64_t cause, cpu_io::TrapContext* ctx) {
    // 直接访问驱动成员变量，无需全局状态
    rx_fifo_.push_back(ReadReg(kRxReg));
    return 0;
  }

  etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)> handler_;
  etl::vector<uint8_t, 256> rx_fifo_;
  uintptr_t base_addr_ = 0;
};
```
### 1.5 delegate 的注意事项

- **对象必须在 delegate 生存期内保持有效**：delegate 持有对象引用（裸指针），对象销毁后 delegate 变为悬空引用
- **不捕获 Lambda 的生命周期**：`delegate::create(lambda_instance)` 接受左值引用，lambda 必须比 delegate 活得长
- **禁止绑定右值**：`delegate::create(std::move(lambda))` 被 deleted，防止悬空

```cpp
// 错误示例
auto MakeHandler() {
  auto lambda = [](uint64_t, cpu_io::TrapContext*) -> uint64_t { return 0; };
  return etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)>::create(lambda);
  // 返回后 lambda 销毁，delegate 悬空！
}
```

---

## 2. etl::fsm — 任务状态机形式化

### 2.1 现状：枚举 + 条件判断

`TaskControlBlock` 当前用枚举 + 分散的条件判断管理状态：

```cpp
// task_control_block.hpp
enum TaskStatus : uint8_t {
  kUnInit, kReady, kRunning, kSleeping, kBlocked, kExited, kZombie
};

TaskStatus status = TaskStatus::kUnInit;
```

状态转换逻辑分散在多个文件中（`schedule.cpp`、`clone.cpp`、`exit.cpp`、`sleep.cpp`、`block.cpp`、`wakeup.cpp`），缺乏统一视图，容易出现非法转换（如从 `kZombie` 直接转到 `kRunning`）。

### 2.2 etl::fsm 简介

`etl::fsm` 是**消息驱动的分层有限状态机**框架：

- 状态是类（继承自 `etl::fsm_state`），可以有进入/退出动作
- 转换由"消息（Message）"触发，类型安全
- 非法转换在编译期或调试期可被发现
- 支持分层 FSM（HFSM），子状态可继承父状态的默认处理

### 2.3 架构概述

```
etl::fsm（宿主机）
  ├── etl::fsm_state<FSM, Context, STATE_ID, Msg1, Msg2, ...>（状态基类）
  │   ├── on_enter_state()  — 进入时调用
  │   ├── on_exit_state()   — 退出时调用
  │   └── on_event(Msg&)    — 消息处理（每种消息一个重载）
  └── receive(message)      — 派发消息，触发当前状态的处理函数
```

### 2.4 将 TaskStatus 建模为 etl::fsm

**步骤 1：定义消息类型（触发转换的事件）**

消息 ID 必须全局唯一（`etl::message<ID>` 要求）。将所有消息 ID **集中定义**在一个头文件中：

```cpp
// src/task/include/task_messages.hpp  —— 消息 ID 统一管理文件
#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_

#include <etl/message.h>

// 所有 FSM 消息 ID 连续分配、不得重复
// 添加新消息时在此文件末尾顶部分配 ID
namespace task_msg_id {
  inline constexpr etl::message_id_t kSchedule = 1;
  inline constexpr etl::message_id_t kYield    = 2;
  inline constexpr etl::message_id_t kSleep    = 3;
  inline constexpr etl::message_id_t kBlock    = 4;
  inline constexpr etl::message_id_t kWakeup   = 5;
  inline constexpr etl::message_id_t kExit     = 6;
  inline constexpr etl::message_id_t kReap     = 7;
  // 未来新增：在此继续分配...
}  // namespace task_msg_id

#include "task_control_block.hpp"

// 消息定义（使用上面的常量 ID）
struct MsgSchedule : etl::message<task_msg_id::kSchedule> {};
struct MsgYield    : etl::message<task_msg_id::kYield>    {}; // 对应抢占或主动放弃 CPU

struct MsgSleep    : etl::message<task_msg_id::kSleep> {
  uint64_t wake_tick;
};
struct MsgBlock    : etl::message<task_msg_id::kBlock> {
  ResourceId resource;
};
struct MsgWakeup   : etl::message<task_msg_id::kWakeup>   {};
struct MsgExit     : etl::message<task_msg_id::kExit> {
  int exit_code;
  bool has_parent;  // 新增：是否具有父进程
};
struct MsgReap     : etl::message<task_msg_id::kReap>     {};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
```

> **规范**：所有 `etl::message<ID>` 的 ID 必须来自 `task_messages.hpp`。
> 禁止在各处高改整数字。
**步骤 2：定义状态类**

```cpp
#include <etl/fsm.h>

// 前向声明 FSM 宿主类
class TaskFsm;

// kReady 状态
class StateReady : public etl::fsm_state<TaskFsm, TaskControlBlock,
                                         TaskStatus::kReady,
                                         MsgSchedule> {
 public:
  // 响应调度消息 → 转移到 Running
  etl::fsm_state_id_t on_event(const MsgSchedule&) {
    return TaskStatus::kRunning;  // 返回目标状态 ID
  }

  // 任何未处理消息的默认处理
  etl::fsm_state_id_t on_event_unknown(const etl::imessage&) {
    return STATE_ID;  // 保持当前状态
  }

  void on_enter_state() {
    // 进入就绪状态时的操作（例如添加到就绪队列）
  }
};

// kRunning 状态
class StateRunning : public etl::fsm_state<TaskFsm, TaskControlBlock,
                                           TaskStatus::kRunning,
                                           MsgYield, MsgSleep, MsgBlock, MsgExit> {
 public:
  etl::fsm_state_id_t on_event(const MsgYield&)  { return TaskStatus::kReady; }
  etl::fsm_state_id_t on_event(const MsgSleep& m) {
    get_fsm_context().sched_info.wake_tick = m.wake_tick;
    return TaskStatus::kSleeping;
  }
  etl::fsm_state_id_t on_event(const MsgBlock& m) {
    get_fsm_context().blocked_on = m.resource;
    return TaskStatus::kBlocked;
  }
  etl::fsm_state_id_t on_event(const MsgExit& m) {
    get_fsm_context().exit_code = m.exit_code;
    // 根据是否有父进程决定进入 Zombie 还是 Exited 状态
    return m.has_parent ? TaskStatus::kZombie : TaskStatus::kExited;
  }
  etl::fsm_state_id_t on_event_unknown(const etl::imessage&) { return STATE_ID; }
};

// kUnInit 状态
class StateUnInit : public etl::fsm_state<TaskFsm, TaskControlBlock,
                                          TaskStatus::kUnInit,
                                          MsgSchedule> {
 public:
  etl::fsm_state_id_t on_event(const MsgSchedule&) { return TaskStatus::kReady; }
  etl::fsm_state_id_t on_event_unknown(const etl::imessage&) { return STATE_ID; }
};

// kSleeping 状态
class StateSleeping : public etl::fsm_state<TaskFsm, TaskControlBlock,
                                            TaskStatus::kSleeping,
                                            MsgWakeup> {
 public:
  etl::fsm_state_id_t on_event(const MsgWakeup&) { return TaskStatus::kReady; }
  etl::fsm_state_id_t on_event_unknown(const etl::imessage&) { return STATE_ID; }
};

// kBlocked 状态
class StateBlocked : public etl::fsm_state<TaskFsm, TaskControlBlock,
                                           TaskStatus::kBlocked,
                                           MsgWakeup> {
 public:
  etl::fsm_state_id_t on_event(const MsgWakeup&) { return TaskStatus::kReady; }
  etl::fsm_state_id_t on_event_unknown(const etl::imessage&) { return STATE_ID; }
};

// kZombie 状态
class StateZombie : public etl::fsm_state<TaskFsm, TaskControlBlock,
                                          TaskStatus::kZombie,
                                          MsgReap> {
 public:
  etl::fsm_state_id_t on_event(const MsgReap&) {
    // 任务从表中移除，状态机停止
    return etl::fsm_state_id_t(0);
  }
  etl::fsm_state_id_t on_event_unknown(const etl::imessage&) { return STATE_ID; }

  void on_enter_state() {
    // TODO: 处理 ReparentChildren()、LeaveThreadGroup()、Wakeup(parent)
  }
};

// kExited 状态
class StateExited : public etl::fsm_state<TaskFsm, TaskControlBlock,
                                          TaskStatus::kExited,
                                          MsgReap> {
 public:
  etl::fsm_state_id_t on_event(const MsgReap&) { return etl::fsm_state_id_t(0); }
  etl::fsm_state_id_t on_event_unknown(const etl::imessage&) { return STATE_ID; }

  void on_enter_state() {
    // TODO: 处理立即资源清理（无父进程需要通知）
  }
};

```

**步骤 3：定义 FSM 宿主**

```cpp
// TaskFsm 宿主：继承 etl::fsm，持有 TCB 作为上下文
class TaskFsm : public etl::fsm {
 public:
  static constexpr etl::message_router_id_t kTaskFsmRouterId = 1;

  explicit TaskFsm(TaskControlBlock& tcb)
    : etl::fsm(kTaskFsmRouterId),
      context_(tcb) {}

  TaskControlBlock& get_context() { return context_; }

  void Start() {
    etl::ifsm_state* state_list[] = {
      &state_uninit_, &state_ready_, &state_running_,
      &state_sleeping_, &state_blocked_, &state_zombie_, &state_exited_
    };
    set_states(state_list, ETL_OR_STD17::size(state_list));
    start();
  }

 private:
  TaskControlBlock& context_;
  StateUnInit   state_uninit_;
  StateReady    state_ready_;
  StateRunning  state_running_;
  StateSleeping state_sleeping_;
  StateBlocked  state_blocked_;
  StateZombie   state_zombie_;
  StateExited   state_exited_;
};
```

**步骤 4：触发状态转换**

```cpp
// 替换分散在各文件中的 task->status = kXxx 为 FSM 消息
void TaskManager::Schedule() {
  // 替换前
  current_task->status = TaskStatus::kReady;

  // 替换后
  current_task->fsm.receive(MsgYield{});  // 对应 schedule.cpp 中的抢占路径

}

void TaskManager::Sleep(uint64_t ms) {
  uint64_t wake_tick = GetCurrentTick() + ms * kTicksPerMs;
  // 替换前
  current_task->status = TaskStatus::kSleeping;
  current_task->sched_info.wake_tick = wake_tick;

  // 替换后（消息携带参数，状态类内部设置 wake_tick）
  current_task->fsm.receive(MsgSleep{.wake_tick = wake_tick});
}
```

### 2.5 迁移成本评估

**优势：**
- 状态转换逻辑集中在状态类中，无需在多个文件中搜索
- 非法转换（未处理消息）通过 `on_event_unknown` 统一拦截
- 进入/退出动作确保状态不变量（如：进入 kReady 一定添加到调度队列）

**成本：**
- `etl::fsm` 依赖 `etl::message_router`，需要全局消息路由器配置
- 需要为每个状态单独编写类，代码量显著增加
- `static` 状态对象不支持多实例（多个 TCB 共享同一状态类对象）
  - 解决方案：使用 CRTP context（`get_fsm_context()` 返回当前 TCB）
- `etl::fsm` 的模板参数（消息列表）有上限（默认 16 个），超出需重新生成 `fsm.h`

**结论：建议使用 `etl::fsm` 处理 `TaskStatus` 状态转换。**

- 状态转换逻辑集中在状态类中，消除分散在多个文件中的隐式赋值
- 非法转换通过 `on_event_unknown` 统一拦截，不会静默失败
- 进入/退出动作确保状态不变量（如进入 kReady 一定添加到调度队列）

迁移注意事项：
- `static` 状态对象不支持多实例，必须用 `get_fsm_context()` 返回当前 TCB——这是 ETL FSM 的标准用法
- `etl::fsm` 依赖 `etl::message_router`，需要配置全局消息路由器，详见 `etl_advanced.md`
---

## 3. etl::flags / etl::bitset — 类型安全位标志

### 3.1 问题：当前的位运算

`task_control_block.hpp` 中：

```cpp
// CloneFlags：用 enum + 按位或表达组合标志
enum CloneFlags : uint64_t {
  kCloneAll      = 0,
  kCloneVm       = 0x00000100,
  kCloneFs       = 0x00000200,
  kCloneFiles    = 0x00000400,
  kCloneSighand  = 0x00000800,
  kCloneParent   = 0x00008000,
  kCloneThread   = 0x00010000,
};
CloneFlags clone_flags = kCloneAll;

// CPU 亲和性位掩码：哪些 CPU 可以运行此任务
uint64_t cpu_affinity = UINT64_MAX;  // 默认：所有 CPU
```

当前方式的问题：
- `CloneFlags` 和 `cpu_affinity` 都是 `uint64_t`，可能意外混用（将亲和性传给 clone）
- 没有类型系统保证位操作的正确性
- 检查标志需要手写 `(flags & kCloneVm) != 0`

### 3.2 etl::flags — 强类型位标志

```cpp
#include <etl/flags.h>

// etl::flags<底层类型, 掩码（可选）>
// 仅允许指定掩码内的位被设置，防止意外污染
```

#### 3.2.1 替换 CloneFlags

```cpp
// 用 constexpr uint64_t 定义位值（保留枚举名）
namespace clone_flag {
  inline constexpr uint64_t kVm       = 0x00000100;
  inline constexpr uint64_t kFs       = 0x00000200;
  inline constexpr uint64_t kFiles    = 0x00000400;
  inline constexpr uint64_t kSighand  = 0x00000800;
  inline constexpr uint64_t kParent   = 0x00008000;
  inline constexpr uint64_t kThread   = 0x00010000;
  // 所有有效标志的掩码
  inline constexpr uint64_t kAllMask  = kVm | kFs | kFiles | kSighand | kParent | kThread;
}

// 类型别名：只允许 kAllMask 范围内的位
using CloneFlags = etl::flags<uint64_t, clone_flag::kAllMask>;

// TCB 中
CloneFlags clone_flags{};  // 默认全 0（kCloneAll）
```

**使用示例：**

```cpp
// 设置标志
CloneFlags flags{};
flags.set(clone_flag::kVm, true);       // 设置 kVm 位
flags.set(clone_flag::kThread, true);   // 设置 kThread 位

// 等价于：
CloneFlags flags2{clone_flag::kVm | clone_flag::kThread};

// 检查标志（更清晰）
if (flags.test(clone_flag::kVm)) {
  // 共享地址空间
}

// 替换前
if ((clone_flags & kCloneVm) != 0) { ... }

// 替换后
if (clone_flags.test(clone_flag::kVm)) { ... }

// 清除标志
flags.reset(clone_flag::kVm);          // 清除 kVm
flags.reset();                          // 清除所有

// 多标志组合
bool is_thread = flags.test(clone_flag::kVm) && flags.test(clone_flag::kThread);

// 类型安全：不能将 cpu_affinity（uint64_t）直接赋值给 CloneFlags
// using CpuAffinity = etl::flags<uint64_t>; // 不同类型，编译错误
```

#### 3.2.2 替换 cpu_affinity

```cpp
// CPU 亲和性：最多支持 64 个核心
using CpuAffinity = etl::flags<uint64_t>;  // 无掩码限制，全 64 位可用

// TCB 中
CpuAffinity cpu_affinity{UINT64_MAX};  // 默认：所有 CPU

// 设置可以运行在 CPU 0 和 CPU 2 上
cpu_affinity.reset();
cpu_affinity.set(1ULL << 0, true);  // CPU 0
cpu_affinity.set(1ULL << 2, true);  // CPU 2

// 检查当前 CPU 是否在亲和性集合中
uint32_t current_cpu = cpu_io::GetCurrentCoreId();
bool can_run = cpu_affinity.test(1ULL << current_cpu);

// 类型安全：不能意外将 CloneFlags 传给需要 CpuAffinity 的函数
void SetAffinity(TaskControlBlock* tcb, CpuAffinity mask);
// SetAffinity(tcb, clone_flags);  // 编译错误！不同类型
```

### 3.3 etl::bitset — 固定位宽位集合

`etl::bitset<N>` 是 `std::bitset<N>` 的 ETL 替代，提供更丰富的 API：

```cpp
#include <etl/bitset.h>

// N 位的位集合
etl::bitset<64> cpu_mask;

cpu_mask.set(0);    // 设置第 0 位（CPU 0）
cpu_mask.set(2);    // 设置第 2 位（CPU 2）
cpu_mask.reset(0);  // 清除第 0 位
cpu_mask.test(1);   // 测试第 1 位
cpu_mask.count();   // 已设置的位数
cpu_mask.any();     // 是否有任意位被设置
cpu_mask.all();     // 是否所有位都被设置
cpu_mask.none();    // 是否没有位被设置
cpu_mask.flip(3);   // 翻转第 3 位

// 以 uint64_t 获取原始值（与 cpu_io 接口互操作）
uint64_t raw = cpu_mask.to_ullong();
```

**`etl::flags` vs `etl::bitset` 选择建议：**

| 场景 | 推荐类型 | 原因 |
|---|---|---|
| 命名标志位（CloneFlags） | `etl::flags<T, Mask>` | 可按位值（常量）操作，掩码限制防止意外 |
| 无符号整数位掩码（cpu_affinity） | `etl::flags<uint64_t>` | 与原始 uint64_t 互操作方便 |
| 按位索引的集合（CPU 集合） | `etl::bitset<N>` | 以索引操作更直观，有 `count()`/`any()` |
| 纯粹替换 std::bitset | `etl::bitset<N>` | API 兼容 |

---

## 4. 组合示例：delegate + flags 重构中断注册

以下展示如何将 `etl::delegate` 和 `etl::flags` 组合用于实际驱动注册场景。

假设 RISC-V PLIC 驱动需要为每个外部中断注册处理函数，且每个中断有属性标志（边缘触发/电平触发、高优先级等）：

```cpp
#include <etl/delegate.h>
#include <etl/flags.h>
#include <etl/unordered_map.h>

// 中断属性标志
namespace irq_flag {
  inline constexpr uint32_t kEdgeTrigger  = 0x01;  // 0 = 电平触发
  inline constexpr uint32_t kHighPriority = 0x02;
  inline constexpr uint32_t kShared       = 0x04;  // 允许多个驱动共享此中断
}
using IrqFlags = etl::flags<uint32_t, irq_flag::kEdgeTrigger |
                                       irq_flag::kHighPriority |
                                       irq_flag::kShared>;

// 中断处理委托
using IrqDelegate = etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)>;

// 中断注册表条目
struct IrqEntry {
  IrqDelegate handler;
  IrqFlags    flags;
};

// 使用 etl::unordered_map 存储注册信息（最多 64 个外部中断）
etl::unordered_map<uint32_t, IrqEntry, 64, 128> irq_table;

// 注册接口
Expected<void> RegisterIrq(uint32_t irq, IrqDelegate handler, IrqFlags flags) {
  if (irq_table.full()) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }
  if (irq_table.contains(irq) && !flags.test(irq_flag::kShared)) {
    return std::unexpected(Error(ErrorCode::kAlreadyExists));
  }
  irq_table.insert({irq, IrqEntry{handler, flags}});
  return {};
}

// 驱动中使用
class Ns16550aDriver {
 public:
  auto Probe(DeviceNode& node) -> Expected<void> {
    uint32_t irq = node.GetIrq(0);

    // 绑定成员函数为 delegate（无全局变量！）
    auto handler = IrqDelegate::create<Ns16550aDriver, &Ns16550aDriver::OnIrq>(*this);

    // 注册：电平触发，非共享
    IrqFlags flags{irq_flag::kEdgeTrigger};  // 仅 edge trigger 位
    return RegisterIrq(irq, handler, flags);
  }

 private:
  uint64_t OnIrq(uint64_t cause, cpu_io::TrapContext* ctx) {
    // 直接访问驱动成员变量，无需全局指针
    rx_fifo_.push_back(ReadReg(kRxReg));
    return 0;
  }

  etl::vector<uint8_t, 256> rx_fifo_;
  uintptr_t base_addr_ = 0;
};
```

---

## 5. 不建议使用的 ETL 功能

### 5.1 etl::fsm 的 start()/reset() —— **可以使用**，注意校验配置

`start()` 和 `reset()` 本身的功能逻辑**与 `ETL_NO_CHECKS` 无关**，在 `ETL_NO_CHECKS = 1` 下也可以正常使用。

`ETL_NO_CHECKS` 只影响运行时**校验断言**（`ETL_ASSERT`），不影响状态机的驱动逻辑：

| 配置 | `start()`/`reset()` 功能 | `set_states()` 校验 | 备注 |
|---|---|---|---|
| `ETL_NO_CHECKS=1`（旧配置） | ✅ 正常 | ❌ 静默跳过 | 状态列表配置错误不报告 |
| `ETL_NO_CHECKS=0`，无其他宏，非 debug | ✅ 正常 | ❌ 仍是 no-op | release build 默认行为 |
| `ETL_NO_CHECKS=0` + `ETL_USE_ASSERT_FUNCTION` | ✅ 正常，且校验生效 | ✅ 调用注册的错误回调 | **推荐内核配置** |

**推荐做法**：将 `ETL_NO_CHECKS=0` 与 `ETL_USE_ASSERT_FUNCTION` 结合，注册内核级错误处理回调：

```cpp
// src/include/etl_profile.h
#define ETL_CPP23_SUPPORTED 1
// ETL_NO_CHECKS 不再定义，启用运行时校验
#define ETL_USE_ASSERT_FUNCTION 1   // 使用自定义错误回调
```

```cpp
// src/main.cpp 或内核初始化早期（ArchInit 之后）
#include <etl/error_handler.h>

static void KernelEtlAssertHandler(const etl::exception& e) {
  // 内核日志输出错误信息，然后 panic
  klog::Err() << "ETL assert: " << e.what() << " at " << e.file_name()
              << ":" << e.line_number();
  // KernelPanic("");  // 或其他 panic 机制
}

// ArchInit 之后调用：
etl::set_assert_function(KernelEtlAssertHandler);
```

> **结论：`start()`/`reset()` 可用。** Section 2 中的使用模式无需修改，
> 只需按上述配置 `ETL_USE_ASSERT_FUNCTION` 即可开启校验。
### 5.2 etl::task（调度框架）—— 不适用

ETL 自带一个简单的合作式任务调度器 `etl::task`，但 SimpleKernel 已有完整的抢占式多核调度器（CFS/FIFO/RR），无需使用 ETL 的调度框架。

### 5.3 etl::mutex / etl::semaphore —— 不适用

ETL 的互斥原语依赖平台层（需要提供 `etl::mutex_base` 的平台实现）。SimpleKernel 的 `SpinLock` 和 `Mutex` 深度依赖 `cpu_io::GetCurrentCoreId()` 和任务阻塞机制，不应被替换。

### 5.4 etl::observer / etl::message_router —— 按需使用

`etl::observer`（发布-订阅）和 `etl::message_router` 是有用的模式，但引入了额外的架构层次。适合设备事件广播（如"UART 接收到数据"通知多个订阅者），但应作为可选增强而非全局重构。

---
