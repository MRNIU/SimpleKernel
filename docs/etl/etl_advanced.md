# ETL 高级机制决策指南

> 本文档覆盖五个 ETL 机制，逐一分析其在 SimpleKernel 中的适用性，并给出明确的使用建议。

---

## 目录

1. [etl::expected vs std::expected](#1-etlexpected-vs-stdexpected)
2. [etl::fsm 的 start() / stop() / reset() 生命周期](#2-etlfsm-的-start--stop--reset-生命周期)
3. [etl::task（合作式调度器任务）](#3-etltask合作式调度器任务)
4. [etl::mutex / etl::semaphore](#4-etlmutex--etlsemaphore)
5. [etl::observer / etl::message_router](#5-etlobserver--etlmessage_router)
6. [集成路线图](#6-集成路线图)

---

## 1. `etl::expected` vs `std::expected`

### 结论：**不替换——继续使用 `std::expected`（C++23）**

---

### 1.1 背景

`etl::expected<TValue, TError>` 是 ETL 对 C++23 `std::expected` 的移植实现，目的是让 C++03/11 时代的嵌入式项目也能使用 Expected 错误处理模式。

SimpleKernel 的 `etl_profile.h` 声明了：

```cpp
#define ETL_CPP23_SUPPORTED 1
```

项目 `src/include/expected.hpp` 已经直接封装 `std::expected`：

```cpp
// src/include/expected.hpp
#include <expected>
template <typename T, typename E = Error>
using Expected = std::expected<T, E>;
using Unexpected = std::unexpected<Error>;
```

---

### 1.2 API 对比

| 功能 | `std::expected` (C++23) | `etl::expected` |
|------|------------------------|----------------|
| `has_value()` | ✅ | ✅ |
| `value()` | ✅ | ✅ |
| `error()` | ✅ | ✅ |
| `operator*` / `operator->` | ✅ | ✅ |
| `value_or()` | ✅ | ✅ |
| `and_then()` / `or_else()` | ✅ (C++23) | ❌ 不支持 |
| `transform()` | ✅ (C++23) | ❌ 不支持 |
| 内部存储 | 编译器优化 | `etl::variant`（额外开销）|
| constexpr 支持 | ✅ 完整 | 部分（受 ETL_CONSTEXPR14 限制）|

---

### 1.3 为什么不替换

1. **API 倒退**：`etl::expected` 不支持 `and_then` / `or_else` / `transform`，这些是 C++23 的核心链式调用 API。
2. **额外间接层**：内部用 `etl::variant` 存储，比编译器原生优化的 `std::expected` 多一层开销。
3. **无迁移收益**：项目已全面使用 `std::expected`，只有跨编译器移植到 C++11 环境时才需要 `etl::expected`。

---

### 1.4 正确做法

继续使用项目封装：

```cpp
#include "expected.hpp"

auto ReadConfig() -> Expected<Config, Error> {
    if (/* ok */) {
        return Config{...};
    }
    return Unexpected{Error::kNotFound};
}

// 使用 and_then 链式调用（etl::expected 不支持此方式）
auto result = ReadConfig()
    .and_then([](Config cfg) -> Expected<int, Error> {
        return cfg.value;
    });
```

---

## 2. `etl::fsm` 的 `start()` / `stop()` / `reset()` 生命周期

### 结论：**建议使用——用 `start()`/`reset()` 管理 FSM 生命周期**

（`etl::fsm` 整体迁移决策见 `etl_delegate_fsm.md`，本节专注生命周期管理。）

---

### 2.1 生命周期 API

```cpp
// start() — 启动 FSM，只能调用一次，幂等
// call_on_enter_state=true（默认）：进入第一个状态时触发 on_enter_state()
virtual void start(bool call_on_enter_state = true);

// is_started() — 检查 FSM 是否已启动
bool is_started() const;           // p_state != nullptr

// reset() — 重置 FSM 到 pre-started 状态（相当于"stop"）
// call_on_exit_state=false（默认）：不触发 on_exit_state()
virtual void reset(bool call_on_exit_state = false);
```

> **注意**：ETL FSM 没有独立的 `stop()` 方法，使用 `reset()` 实现停止语义。

---

### 2.2 启动流程

```
start(call_on_enter_state=true)
  │
  ├─ p_state = state_list[0]          ← 第一个注册状态
  │
  └─ do {
        next_id = p_state->on_enter_state()
        if (next_id != No_State_Change)
            p_state = state_list[next_id]  ← 允许在 on_enter 时立即跳转
     } while (p_last_state != p_state)    ← 直到状态稳定
```

`on_enter_state()` 可以触发即时状态跳转（用于初始化时的条件分支），循环直到状态稳定。

---

### 2.3 状态回调

每个 `etl::fsm_state<TContext, StateId, ParentId>` 必须实现：

```cpp
// 进入状态时调用，返回值决定是否立即跳转
virtual fsm_state_id_t on_enter_state() { return No_State_Change; }

// 离开状态时调用
virtual void on_exit_state() {}
```

---

### 2.4 SimpleKernel 中的应用模式

以 `TaskManager` 的任务状态机为例：

```cpp
// src/task/include/task_control_block.hpp
// TaskStatus 状态：kUnInit, kReady, kRunning, kSleeping, kBlocked, kExited, kZombie
// 完整转换：kUnInit → kReady → kRunning → kSleeping/kBlocked/kZombie/kExited

// 消息 ID 集中管理（见 src/task/include/task_messages.hpp）
namespace task_msg_id {
    inline constexpr etl::message_id_t kSchedule = 1;
    inline constexpr etl::message_id_t kYield    = 2;
    inline constexpr etl::message_id_t kSleep    = 3;
    inline constexpr etl::message_id_t kBlock    = 4;
    inline constexpr etl::message_id_t kWakeup   = 5;
    inline constexpr etl::message_id_t kExit     = 6;
    inline constexpr etl::message_id_t kReap     = 7;
}

class TaskFsm : public etl::fsm {
public:
    TaskFsm() : etl::fsm(kRouterId) {}

    void Init(etl::ifsm_state* const state_list[], size_t num_states) {
        set_states(state_list, num_states);
        start();   // 触发 kReady.on_enter_state()
    }

    void Stop() {
        reset(/*call_on_exit_state=*/true);  // 触发 on_exit_state() 做清理
    }

private:
    static constexpr etl::message_router_id_t kRouterId = 1;
};
```

---

### 2.5 在构造函数中调用 `start()` 是否安全？

**结论：不建议在构造函数中调用 `start()`，应在显式 `Init()` 中调用。**

原因分析：

```cpp
// ETL fsm 构造函数只做零初始化，本身完全安全：
fsm(etl::message_router_id_t id)
  : imessage_router(id)
  , p_state(ETL_NULLPTR)       // 安全
  , state_list(ETL_NULLPTR)    // 安全
  , number_of_states(0U)       // 安全
  , is_processing_state_change(false) {}

// set_states() 需要状态对象已就位（调用 state_list[i]->set_fsm_context(*this)）
// start() 会立即调用 on_enter_state()——用户代码，可能访问 klog、设备、调度器
```

SimpleKernel 的初始化顺序（`main.cpp`）：

```
ArchInit → MemoryInit → InterruptInit → DeviceInit → FileSystemInit
    → TaskManagerSingleton::instance().InitCurrentCore()
```

全局静态对象的构造函数在 `ArchInit` 之前（全局构造阶段），此时 `klog`、设备、调度器均未就绪。

| 调用时机 | 是否安全 | 原因 |
|---------|---------|------|
| 全局静态对象的构造函数 | ❌ 危险 | `on_enter_state()` 可能调用未初始化的子系统 |
| 成员变量（类内构造） | ❌ 危险 | 同上，依赖外部子系统的风险相同 |
| 显式 `Init()` 方法中 | ✅ 安全 | 调用方保证所有依赖已初始化 |
| `etl::singleton<T>::create()` 中 | ✅ 安全 | Singleton 在 `main()` 中显式触发 |

**推荐模式：**

```cpp
class TaskFsm : public etl::fsm {
public:
    // 构造函数只做 FSM ID 绑定，不调用 start()
    TaskFsm() : etl::fsm(kRouterId) {}

    // 在所有依赖就绪后显式调用
    void Init() {
        set_states(states_, etl::size(states_));
        start();  // 此时 klog、调度器等已就绪
    }

    void Shutdown() {
        reset(/*call_on_exit_state=*/true);
    }

private:
    static constexpr etl::message_router_id_t kRouterId = 1;
    StateUnInit   state_uninit_;
    StateReady    state_ready_;
    StateRunning  state_running_;
    StateSleeping state_sleeping_;
    StateBlocked  state_blocked_;
    StateZombie   state_zombie_;
    StateExited   state_exited_;
    etl::ifsm_state* states_[7] = {
        &state_uninit_, &state_ready_, &state_running_,
        &state_sleeping_, &state_blocked_, &state_zombie_, &state_exited_
    };
```

---

### 2.6 重入保护

ETL FSM 内置重入保护：

```cpp
// 内部使用 fsm_reentrancy_guard
// 若在 on_enter_state/on_exit_state/process_event 内部再次调用 start/receive/reset
// 会触发 fsm_reentrant_exception（ETL_NO_CHECKS=1 时静默失败）
```

**使用规则**：不在状态回调中直接调用 `start()` 或 `reset()`。状态跳转通过返回新的 `state_id` 完成。

---

### 2.7 迁移步骤

1. 为每个需要状态管理的模块创建 `XxxFsm` 类（继承 `etl::fsm`）
2. 构造函数只绑定 FSM ID，**不调用 `start()`**
3. 在模块显式初始化（`Init()`）中调用 `set_states()` 然后 `start()`
4. 在模块关闭（`Shutdown()`/析构）中调用 `reset(true)`
5. 消息 ID 统一在 `src/task/include/task_messages.hpp` 中定义
---

## 3. `etl::task`（合作式调度器任务）

### 结论：**不适用——与 SimpleKernel 调度器架构冲突**

---

### 3.1 etl::task 是什么

`etl::task` 是 ETL 为**合作式（cooperative）单线程调度器**设计的任务基类：

```cpp
class etl::task {
public:
    // 返回本轮需要处理的工作量（0 = 无工作）
    virtual uint32_t task_request_work() const = 0;

    // 执行一个工作单元
    virtual void task_process_work() = 0;

    // 加入调度器时的回调
    virtual void on_task_added() {}

    // 优先级（高值 = 高优先级）
    task_priority_t get_task_priority() const;

    // 任务运行状态标记
    void set_task_running(bool);
    bool task_is_running() const;
};
```

配套的 `etl::task_scheduler` 是一个**单线程轮询循环**：

```cpp
// ETL task_scheduler 伪代码
while (true) {
    for (auto& task : tasks) {
        if (task.task_request_work() > 0) {
            task.task_process_work();
        }
    }
}
```

---

### 3.2 与 SimpleKernel 的冲突

| 维度 | `etl::task_scheduler` | SimpleKernel 调度器 |
|------|----------------------|-------------------|
| 调度模式 | 合作式（cooperative） | 抢占式（preemptive） |
| 多核支持 | ❌ 单线程，无同步原语 | ✅ 多核，SpinLock 保护 |
| 调度算法 | 优先级轮询 | CFS / FIFO / RR |
| TCB | 无（无上下文切换） | `TaskControlBlock`（寄存器上下文） |
| 阻塞/唤醒 | ❌ 不支持 | ✅ `kBlocked`/`kReady` 状态 |
| 时钟中断驱动 | ❌ 不支持 | ✅ Timer Interrupt 触发抢占 |

`etl::task` 本质是一个**裸轮询框架**，没有上下文切换、没有调度点、没有阻塞语义，无法替代也无法嵌入 SimpleKernel 现有的任务管理体系。

---

### 3.3 正确做法

继续使用 `src/task/` 中的抢占式调度器：

- `SchedulerBase` → `CfsScheduler` / `FifoScheduler` / `RrScheduler`
- `TaskControlBlock` 管理任务上下文
- `TaskManagerSingleton::instance()` 统一入口
如果需要"轻量级周期性工作"，在 Timer Interrupt 回调中触发即可，无需引入合作式调度。

---

## 4. `etl::mutex` / `etl::semaphore`

### 结论：**不可用——平台绑定，SimpleKernel 无对应 OS 定义**

---

### 4.1 etl::mutex 的平台分发机制

`etl/mutex.h` 是一个**平台分发头文件**，根据宏定义选择实现：

```cpp
#if defined(ETL_TARGET_OS_CMSIS_OS2)
    #include "mutex/mutex_cmsis_os2.h"
    #define ETL_HAS_MUTEX 1
#elif defined(ETL_TARGET_OS_FREERTOS)
    #include "mutex/mutex_freertos.h"
    #define ETL_HAS_MUTEX 1
#elif defined(ETL_TARGET_OS_THREADX)
    #include "mutex/mutex_threadx.h"
    #define ETL_HAS_MUTEX 1
#elif defined(ETL_USING_STL) && ETL_USING_CPP11
    #include "mutex/mutex_std.h"       // std::mutex
    #define ETL_HAS_MUTEX 1
#elif defined(ETL_COMPILER_ARM)
    #include "mutex/mutex_arm.h"
    #define ETL_HAS_MUTEX 1
#elif defined(ETL_COMPILER_GCC)
    #include "mutex/mutex_gcc_sync.h"  // __sync_*
    #define ETL_HAS_MUTEX 1
#elif defined(ETL_COMPILER_CLANG)
    #include "mutex/mutex_clang_sync.h"
    #define ETL_HAS_MUTEX 1
#else
    #define ETL_HAS_MUTEX 0            // ← SimpleKernel 走这里
#endif
```

SimpleKernel 的 `etl_profile.h` 没有定义任何 `ETL_TARGET_OS_*`，也没有定义 `ETL_USING_STL`，因此 `ETL_HAS_MUTEX = 0`。

**`etl::mutex` 在当前配置下编译后是一个空壳，不提供任何同步语义。**

---

### 4.2 etl::semaphore

ETL 当前版本（截至本文档编写时）**没有 `etl/semaphore.h`**，信号量功能不存在。

---

### 4.3 为什么不添加 ETL_TARGET_OS 定义

SimpleKernel 是**独立（freestanding）内核**，它本身就是操作系统——它不运行在任何操作系统之上。`etl::mutex` 的各个平台实现（FreeRTOS、CMSIS、ThreadX）都依赖 OS 提供的调度原语，而 SimpleKernel 本身就是提供这些原语的一层。

即使强行使用 `ETL_COMPILER_GCC` 走 `__sync_*` 路径，`__sync_*` 在多核环境下也缺少内存屏障的完整语义，不适合内核级别的同步需求。

---

### 4.4 正确做法：使用项目现有同步原语

| 需求 | 使用 | 位置 |
|------|------|------|
| 自旋等待（短临界区）| `SpinLock` + `LockGuard<SpinLock>` | `src/include/spinlock.hpp` |
| 阻塞等待（可睡眠）| `Mutex` | `src/include/mutex.hpp` + `src/task/mutex.cpp` |
| 原子操作 | `__atomic_*` / `__sync_*` builtins | 内联汇编 |

```cpp
// 自旋锁用法（中断处理、短临界区）
SpinLock lock_;

void CriticalSection() {
    LockGuard<SpinLock> guard(lock_);
    // 临界区代码
}

// 互斥锁用法（可睡眠任务间同步）
Mutex mutex_;

void BlockingSection() {
    mutex_.Lock();
    // 临界区代码
    mutex_.Unlock();
}
```

---

## 5. `etl::observer` / `etl::message_router`

### 结论：**有条件使用**
- `etl::observer`：**建议用于同步事件广播**——时钟节拍分发、Panic 广播、硬件事件通知
- `etl::message_router`：**建议用于跨上下文异步消息路由**——中断底半部、线程生命周期总线、协议栈层间解耦

---

### 5.1 etl::observer — 同步一对多广播

**核心特点：同步调用、快速执行、一对多广播。适用于硬件级事件或底层全局状态变化。**

#### API

```cpp
// observable: 被观察对象（Subject）
template <typename TObserver, const size_t Max_Observers>
class observable {
    void add_observer(TObserver& observer);
    bool remove_observer(TObserver& observer);
    void enable_observer(TObserver& observer, bool state = true);
    void disable_observer(TObserver& observer);
    void clear_observers();
    size_type number_of_observers() const;

    template <typename TNotification>
    void notify_observers(TNotification n);  // 通知所有已启用的 observer
    void notify_observers();                 // 无参数版本
};

// observer: 观察者（C++11 变参模板，支持多通知类型）
template <typename T1, typename... TRest>
class observer : public observer<T1>, public observer<TRest...> {
    virtual void notification(T1) = 0;
};
```

#### 关键特性

- **静态容量**：`Max_Observers` 编译期固定，无堆分配
- **可启用/禁用**：`enable_observer(obs, false)` 临时屏蔽某个 observer，无需移除
- **多通知类型**：一个 observer 可订阅多种通知：`class MyObserver : public etl::observer<TickEvent, PanicEvent> {...}`
- **同步调用**：`notify_observers` 在调用线程（或中断）上下文中直接执行，无队列缓冲

---

#### 场景 1：系统时钟节拍（Tick）分发

**背景**：在 aarch64（Generic Timer）或 riscv64（CLINT/Core Local Timer）中，硬件定时器周期性触发中断。中断处理函数不应硬编码对各模块的调用。

```cpp
// 通知类型
struct TickEvent { uint64_t jiffies; };
using ITickObserver = etl::observer<TickEvent>;

// Subject：时钟中断处理
class TimerInterruptHandler : public etl::observable<ITickObserver,
                                                      kernel_config::kTickObservers> {
public:
    void OnInterrupt() {
        ++jiffies_;
        notify_observers(TickEvent{jiffies_});  // 广播给所有订阅者
    }
private:
    uint64_t jiffies_ = 0;
};

// Observer 1：调度器（提议的观察者模式，当前由 TaskManager::TickUpdate 直接调用）
class CfsScheduler : public SchedulerBase, public ITickObserver {
    void notification(TickEvent evt) override {
        OnTick(evt.jiffies);
    }
    // ... 现有实现 ...
};

// Observer 2：睡眠队列——唤醒到期任务
class SleepQueue : public ITickObserver {
    void notification(TickEvent evt) override {
        WakeExpiredTasks(evt.jiffies);
    }
};
```

> **约束**：`notification()` 在中断上下文执行，必须极短，**不能阻塞**（不能请求 SpinLock、Mutex 或做任何可能休眠的操作）。

---

#### 场景 2：内核 Panic 广播

**背景**：内核遇到不可恢复错误（严重缺页、断言失败）时需要安全停机，多个子系统各自做清理。

```cpp
struct PanicEvent { const char* reason; uint64_t pc; };
using IPanicObserver = etl::observer<PanicEvent>;

class KernelPanic : public etl::observable<IPanicObserver,
                                           kernel_config::kPanicObservers> {
public:
    [[noreturn]] void Trigger(const char* reason, uint64_t pc) {
        notify_observers(PanicEvent{reason, pc});
        // 关中断，停机
        cpu_io::DisableInterrupts();
        while (true) { cpu_io::Pause(); }
    }
};

// Observer：文件系统——尝试同步缓存，防止数据损坏
class VfsLayer : public IPanicObserver {
    void notification(PanicEvent) override { FlushAll(); }
};

// Observer：看门狗——停止喂狗，触发硬件复位
class WatchdogDriver : public IPanicObserver {
    void notification(PanicEvent) override { StopFeeding(); }
};
```

> **注意**：Panic 路径中 observer 的执行顺序是注册顺序，若某个 `notification()` 崩溃会中断后续通知。保持每个回调极简。

---

#### 场景 3：UART 接收通知（设备驱动）

```cpp
struct UartRxEvent { char data; };
using IUartObserver = etl::observer<UartRxEvent>;

class Ns16550aDriver : public etl::observable<IUartObserver,
                                              kernel_config::kUartObservers> {
public:
    void OnInterrupt() {
        char c = ReadRegister(kRBR);
        notify_observers(UartRxEvent{c});
    }
};

class DebugConsole : public IUartObserver {
    void notification(UartRxEvent evt) override { ProcessChar(evt.data); }
};
```

---

### 5.2 etl::message_router — 跨上下文异步路由

**核心特点：带数据载荷（Payload）、跨上下文边界、多对多解耦。适用于中断底半部、跨子系统事件、协议栈层间传递。**

#### 在 etl::fsm 中的角色

`etl::fsm` 继承自 `etl::imessage_router`，**引入 `etl::fsm` 即自动引入 `etl::message_router`**。

```
etl::imessage_router          ← 消息路由基类（有路由 ID，可转发消息）
    └── etl::fsm              ← FSM 是一种特殊的消息路由器
```

#### 路由器 ID 分配

```cpp
// 保留 ID（不可使用）
// 251 = MESSAGE_ROUTER, 252 = MESSAGE_BROKER
// 253 = ALL_MESSAGE_ROUTERS, 254 = MESSAGE_BUS, 255 = NULL_MESSAGE_ROUTER

// 用户可用范围：0–250
// 统一在 src/task/include/task_messages.hpp 中分配
namespace router_id {
    constexpr etl::message_router_id_t kTimerHandler  = 0;
    constexpr etl::message_router_id_t kTaskFsm       = 1;
    constexpr etl::message_router_id_t kVirtioBlk     = 2;
    constexpr etl::message_router_id_t kVirtioNet     = 3;
}
```

---

#### 场景 1：Virtio 中断底半部（Bottom Half）路由

**背景**：Virtio 设备完成 I/O 触发中断（Top Half），但 Virtqueue 解析和缓冲区回收耗时，必须放到线程上下文（Bottom Half）处理。

```
IRQ（Top Half）
  │  构造静态消息 VirtioQueueReadyMsg{device_id, queue_index}
  └─→ etl::send_message(bottom_half_router, msg)
       │  退出中断上下文
       └─→ Worker Thread 的 on_receive(VirtioQueueReadyMsg)
              │
              ├─→ VirtioBlkDriver::ProcessQueue()   （device_id == blk）
              └─→ VirtioNetDriver::ProcessQueue()   （device_id == net）
```

```cpp
// 消息类型
struct VirtioQueueReadyMsg : public etl::message<VirtioQueueReadyMsg> {
    static constexpr etl::message_id_t ID = msg_id::kVirtioQueueReady;
    uint32_t device_id;
    uint32_t queue_index;
};

// 底半部路由器（Worker Thread 持有）
class VirtioBottomHalf : public etl::message_router<VirtioBottomHalf,
                                                     VirtioQueueReadyMsg> {
public:
    VirtioBottomHalf() : message_router(router_id::kVirtioBlk) {}

    void on_receive(const VirtioQueueReadyMsg& msg) {
        if (msg.device_id == kBlkDevId)  blk_driver_.ProcessQueue(msg.queue_index);
        if (msg.device_id == kNetDevId)  net_driver_.ProcessQueue(msg.queue_index);
    }
    void on_receive_unknown(const etl::imessage&) {}
};

// 中断处理（Top Half）——极短，只发消息
void VirtioIrqHandler() {
    static VirtioQueueReadyMsg msg;  // 静态分配，不触碰堆
    msg.device_id   = ReadDeviceId();
    msg.queue_index = ReadQueueIndex();
    etl::send_message(bottom_half_, msg);  // 压入，退出中断
}
```

> **关键**：消息对象必须**静态分配**（或来自预分配池）。中断中不能动态分配内存。

---

#### 场景 2：线程生命周期事件总线

**背景**：线程状态变化（创建、退出）时，内存管理器、父进程管理器、Mutex 子系统都需要响应，但互相不应有头文件依赖。

```cpp
// 消息类型
struct ThreadExitMsg : public etl::message<ThreadExitMsg> {
    static constexpr etl::message_id_t ID = msg_id::kThreadExit;
    uint32_t thread_id;
    int      exit_code;
};

// 内存管理器——回收内核栈和页表
class MemoryManager : public etl::message_router<MemoryManager, ThreadExitMsg> {
    void on_receive(const ThreadExitMsg& msg) { ReclaimThreadResources(msg.thread_id); }
    void on_receive_unknown(const etl::imessage&) {}
};

// Mutex 子系统——检测死锁线程持有的锁
class MutexManager : public etl::message_router<MutexManager, ThreadExitMsg> {
    void on_receive(const ThreadExitMsg& msg) { ReleaseOrphanedLocks(msg.thread_id); }
    void on_receive_unknown(const etl::imessage&) {}
};

// TaskManager 退出时广播（线程上下文，无并发限制）
void TaskManager::Exit(int code) {
    auto* task = GetCurrentTask();
    static ThreadExitMsg msg;
    msg.thread_id = task->id;
    msg.exit_code = code;
    etl::send_message(mem_mgr,   msg);
    etl::send_message(mutex_mgr, msg);
}
```

---

#### 场景 3：网络协议栈层间解耦

**背景**：网卡驱动收到以太网帧后，需要逐层解析（Ethernet → IP → TCP/UDP），各层之间不应有编译期依赖。

```
NicDriver（以太网帧）
  └─→ send_message(router, EthernetFrameMsg)
       └─→ IpLayer::on_receive(EthernetFrameMsg)
              └─→ send_message(router, IpPacketMsg)
                   ├─→ TcpLayer::on_receive(IpPacketMsg{proto=TCP})
                   └─→ UdpLayer::on_receive(IpPacketMsg{proto=UDP})
```

各层只依赖消息类型定义（一个 POD 头文件），不依赖彼此的类头文件。

---

### 5.3 并发安全与上下文约束

| 场景 | observer | message_router |
|------|---------|----------------|
| 纯中断上下文（所有注册在启动时完成） | ✅ 安全，无需锁 | ✅ 安全（消息静态分配） |
| 运行时动态 `add_observer` / `remove_observer` | ⚠️ 需要 `SpinLock` + 关中断 | ⚠️ 路由表修改需加锁 |
| `notification()` / `on_receive()` 耗时操作 | ❌ 中断上下文禁止 | ✅ 用路由器将消息转入线程上下文 |
| `notification()` / `on_receive()` 调用 `Mutex::Lock()` | ❌ 禁止（中断中死锁） | ✅ 在线程上下文可以 |

**规则总结**：
1. `etl::observer`（`notify_observers`）在中断中调用时，所有 `notification()` 必须是纯计算或原子写，不能阻塞。
2. 若处理耗时，用 `etl::message_router` 将消息转发到工作线程，Bottom Half 中处理。
3. 运行时动态注册观察者（模块热加载等）需要 `LockGuard<SpinLock>` 保护 observer 列表，并在操作期间关本地中断（`cpu_io::DisableLocalInterrupts()`）。

---

### 5.4 容量常量管理

所有 `Max_Observers` 统一在 `src/include/kernel_config.hpp` 定义：

// src/include/kernel_config.hpp
// 以下常量需要新增（当前文件仅包含任务/调度器容量常量）
namespace kernel_config {
    // 已有常量：kMaxTasks, kMaxSchedulers 等（见现有文件）
    // 以下为 etl::observer 集成时需要新增的常量：
    inline constexpr size_t kTickObservers   = 8;
    inline constexpr size_t kPanicObservers  = 4;
    inline constexpr size_t kUartObservers   = 4;
    inline constexpr size_t kDeviceObservers = 8;
}

---

## 6. 集成路线图

按依赖关系和风险排序：

| 阶段 | 机制 | 决策 | 前置条件 | 风险 |
|------|------|------|---------|------|
| **立即** | `etl::expected` | ❌ 不替换 | — | 无 |
| **立即** | `etl::mutex/semaphore` | ❌ 不引入 | — | 无 |
| **立即** | `etl::task` | ❌ 不引入 | — | 无 |
| **Phase 1** | `etl::observer` | ✅ 按需引入 | `kernel_config.hpp` 容量常量（需新增） | 低 |
| **Phase 2** | `etl::fsm` + `start()/reset()` | ✅ 建议引入 | 消息 ID 集中管理（`task_messages.hpp`） | 中 |
| **Phase 3** | `etl::message_router`（总线模式） | 🔄 随 FSM 按需扩展 | FSM 迁移完成 | 中 |

### Phase 1 检查清单（etl::observer）

- [ ] 在 `kernel_config.hpp` 添加 `kXxxObservers` 容量常量
- [ ] 在驱动/子系统头文件中定义通知类型（POD 结构体）
- [ ] Subject 类继承 `etl::observable<IXxxObserver, kernel_config::kXxxObservers>`
- [ ] Observer 类继承对应的 `etl::observer<XxxEvent>`
- [ ] `notification()` 保持简短，不阻塞，不申请锁
- [ ] Observer 对象销毁前调用 `remove_observer` 或 `clear_observers`
- [ ] 若需运行时动态注册，用 `SpinLock` + 关中断保护 `add/remove_observer` 调用

### Phase 2 检查清单（etl::fsm + 生命周期）

- [ ] 创建 `src/task/include/task_messages.hpp`，集中定义所有消息 ID 和路由器 ID
- [ ] FSM 构造函数只绑定 router ID，不调用 `start()`
- [ ] 在显式 `Init()` 中调用 `set_states()` 然后 `start()`
- [ ] 在 `Shutdown()` 中调用 `reset(/*call_on_exit_state=*/true)`
- [ ] 每个状态实现 `on_enter_state()` 和 `on_exit_state()`
- [ ] 不在状态回调中直接调用 `start()` / `reset()`（重入保护）
- [ ] 状态跳转通过 `return new_state_id` 完成，不在回调内用 `transition_to()`

### Phase 3 检查清单（etl::message_router 总线模式）

- [ ] 消息类型继承 `etl::message<MsgType>`，含静态 `ID` 常量
- [ ] 消息对象静态分配或来自预分配池（中断 Top Half 禁止动态分配）
- [ ] Bottom Half 路由器在线程上下文持有，Top Half 只调用 `etl::send_message`
- [ ] 路由器 ID 在 `task_messages.hpp` 统一分配，不硬编码整数

---

*参考文档：*
- *[etl_overview.md](./etl_overview.md) — ETL 整体配置*
- *[etl_delegate_fsm.md](./etl_delegate_fsm.md) — delegate 和 FSM 迁移策略*
- *[etl_containers.md](./etl_containers.md) — 容器容量统一管理*
- *[etl_io_port.md](./etl_io_port.md) — io_port 迁移决策*
