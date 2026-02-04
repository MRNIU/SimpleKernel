# Task 模块单元测试环境层设计

## 架构概述

Task 模块的单元测试采用三层架构设计，职责清晰、易于维护：

```
┌─────────────────────────────────────┐
│         测试层 (Test Layer)          │
│  - 编写测试用例                       │
│  - 调用 Mock 层接口                   │
│  - 验证调度行为                       │
└──────────────┬──────────────────────┘
               │ 通过 Mock API
               ↓
┌─────────────────────────────────────┐
│         Mock 层 (Mock Layer)         │
│  - cpu_io.h: 中断、CoreID、页表      │
│  - arch_mock.cpp: switch_to 等       │
│  - 提供硬件接口的模拟实现             │
└──────────────┬──────────────────────┘
               │ 操作环境状态
               ↓
┌─────────────────────────────────────┐
│       环境层 (Environment Layer)     │
│  - TestEnvironmentState: 全局状态    │
│  - CoreEnvironment: 单核状态         │
│  - 记录中断、页表、线程、切换历史     │
└─────────────────────────────────────┘
```

## 核心组件

### 1. 环境层 (Environment Layer)

#### `CoreEnvironment` - 单核环境状态

模拟单个 CPU 核心的运行环境：

```cpp
struct CoreEnvironment {
  size_t core_id;                    // 核心 ID
  bool interrupt_enabled;            // 中断使能状态
  int interrupt_nest_level;          // 中断嵌套深度
  uint64_t page_directory;           // 页表基地址
  bool paging_enabled;               // 分页是否启用
  TaskControlBlock* current_thread;  // 当前运行线程
  TaskControlBlock* idle_thread;     // 空闲线程
  CpuSchedData* sched_data;          // 调度数据
  std::vector<SwitchEvent> switch_history;  // 切换历史
  uint64_t local_tick;               // 本地时钟
  uint64_t total_switches;           // 切换次数统计
  std::thread::id thread_id;         // 关联的 std::thread
};
```

#### `TestEnvironmentState` - 全局环境管理器

单例模式，管理所有核心的环境：

```cpp
class TestEnvironmentState {
public:
  static auto GetInstance() -> TestEnvironmentState&;

  // 核心管理
  void InitializeCores(size_t num_cores);
  void ResetAllCores();
  auto GetCore(size_t core_id) -> CoreEnvironment&;

  // 线程到核心的映射
  void BindThreadToCore(std::thread::id tid, size_t core_id);
  auto GetCoreIdForThread(std::thread::id tid) -> size_t;
  auto GetCurrentCoreEnv() -> CoreEnvironment&;

  // 上下文到任务的映射
  void RegisterTaskContext(void* context_ptr, TaskControlBlock* task);
  auto FindTaskByContext(void* context_ptr) -> TaskControlBlock*;

  // 调试与验证
  void DumpAllCoreStates() const;
  auto GetAllSwitchHistory() const -> std::vector<SwitchEvent>;
  void ClearSwitchHistory();
};
```

### 2. Mock 层 (Mock Layer)

#### `cpu_io.h` - 硬件接口模拟

所有硬件相关的接口都通过环境层实现：

```cpp
namespace cpu_io {

// 核心 ID 查询
inline auto GetCurrentCoreId() -> size_t {
  return test_env::TestEnvironmentState::GetInstance()
      .GetCoreIdForThread(std::this_thread::get_id());
}

// 中断控制
inline void EnableInterrupt() {
  auto& core = test_env::TestEnvironmentState::GetInstance()
      .GetCurrentCoreEnv();
  core.interrupt_enabled = true;
}

inline void DisableInterrupt() {
  auto& core = test_env::TestEnvironmentState::GetInstance()
      .GetCurrentCoreEnv();
  core.interrupt_enabled = false;
}

inline bool GetInterruptStatus() {
  auto& core = test_env::TestEnvironmentState::GetInstance()
      .GetCurrentCoreEnv();
  return core.interrupt_enabled;
}

// 页表操作
namespace virtual_memory {
  inline void SetPageDirectory(uint64_t pd) {
    auto& core = test_env::TestEnvironmentState::GetInstance()
        .GetCurrentCoreEnv();
    core.page_directory = pd;
  }

  inline auto GetPageDirectory() -> uint64_t {
    auto& core = test_env::TestEnvironmentState::GetInstance()
        .GetCurrentCoreEnv();
    return core.page_directory;
  }
}

}  // namespace cpu_io
```

#### `arch_mock.cpp` - 架构相关函数

```cpp
void switch_to(cpu_io::CalleeSavedContext* prev_ctx,
               cpu_io::CalleeSavedContext* next_ctx) {
  auto& env_state = test_env::TestEnvironmentState::GetInstance();
  auto& core = env_state.GetCurrentCoreEnv();

  // 查找任务
  auto* prev_task = env_state.FindTaskByContext(prev_ctx);
  auto* next_task = env_state.FindTaskByContext(next_ctx);

  // 记录切换事件
  core.switch_history.push_back({
    .timestamp = core.local_tick,
    .from = prev_task,
    .to = next_task,
    .core_id = core.core_id,
  });

  // 更新当前线程
  core.current_thread = next_task;
  core.total_switches++;

  // 不执行真实的栈切换（单元测试中）
}
```

### 3. 测试层 (Test Layer)

#### `TaskTestHarness` - 测试 Fixture 基类

所有 Task 模块的测试都应继承自这个类：

```cpp
class TaskTestHarness : public ::testing::Test {
protected:
  void SetUp() override {
    // 1. 重置环境层
    auto& env_state = test_env::TestEnvironmentState::GetInstance();
    env_state.ResetAllCores();
    env_state.InitializeCores(num_cores_);

    // 2. 绑定主测试线程到 core 0
    env_state.BindThreadToCore(std::this_thread::get_id(), 0);

    // 3. 重置 PerCpu 数据
    // 4. 重置 TaskManager
  }

  void TearDown() override {
    // 清理环境
  }

  void SetNumCores(size_t num_cores) { num_cores_ = num_cores; }

  size_t num_cores_ = 1;  // 默认单核
};
```

## 使用示例

### 单核测试

```cpp
class SingleCoreSchedulerTest : public TaskTestHarness {
protected:
  void SetUp() override {
    SetNumCores(1);  // 单核
    TaskTestHarness::SetUp();
  }
};

TEST_F(SingleCoreSchedulerTest, RoundRobinScheduling) {
  auto& env = GetEnvironmentState();

  // 创建任务...
  // 运行调度器...

  // 验证切换历史
  auto history = env.GetAllSwitchHistory();
  ASSERT_EQ(history.size(), expected_switches);

  // 验证切换顺序
  EXPECT_EQ(history[0].from, task1);
  EXPECT_EQ(history[0].to, task2);
}
```

### 多核测试

```cpp
class MultiCoreSchedulerTest : public TaskTestHarness {
protected:
  void SetUp() override {
    SetNumCores(4);  // 四核
    TaskTestHarness::SetUp();
  }
};

TEST_F(MultiCoreSchedulerTest, LoadBalancing) {
  auto& env = GetEnvironmentState();

  // 创建多个工作线程模拟多核
  std::vector<std::thread> workers;
  for (size_t i = 0; i < 4; ++i) {
    workers.emplace_back([&env, i]() {
      // 绑定到指定核心
      env.BindThreadToCore(std::this_thread::get_id(), i);

      // 运行调度器...
    });
  }

  // 等待所有线程完成
  for (auto& t : workers) {
    t.join();
  }

  // 验证各核心的负载
  for (size_t i = 0; i < 4; ++i) {
    auto& core = env.GetCore(i);
    EXPECT_GT(core.total_switches, 0);
  }
}
```

### 验证中断状态

```cpp
TEST_F(TaskTestHarness, InterruptDisabledDuringSwitch) {
  auto& env = GetEnvironmentState();

  // 在任务切换前禁用中断
  cpu_io::DisableInterrupt();

  // 执行切换...

  // 验证环境层状态
  auto& core = env.GetCurrentCoreEnv();
  EXPECT_FALSE(core.interrupt_enabled);
}
```

### 验证页表切换

```cpp
TEST_F(TaskTestHarness, PageTableSwitching) {
  auto& env = GetEnvironmentState();

  const uint64_t task1_pd = 0x1000;
  const uint64_t task2_pd = 0x2000;

  // 设置任务1的页表
  cpu_io::virtual_memory::SetPageDirectory(task1_pd);
  EXPECT_EQ(env.GetCurrentCoreEnv().page_directory, task1_pd);

  // 切换到任务2并设置其页表
  cpu_io::virtual_memory::SetPageDirectory(task2_pd);
  EXPECT_EQ(env.GetCurrentCoreEnv().page_directory, task2_pd);
}
```

## 关键优势

1. **职责分离**: 环境层、Mock层、测试层各司其职，易于维护
2. **可观测性**: 所有硬件状态和切换历史都被记录，便于调试
3. **易于扩展**: 需要模拟新特性时只需扩展 `CoreEnvironment`
4. **线程安全**: 使用互斥锁保护共享数据
5. **测试隔离**: 每个测试的 SetUp/TearDown 确保状态独立

## 扩展点

### 添加新的硬件特性

例如添加 TLB 模拟：

```cpp
struct CoreEnvironment {
  // ... 现有字段 ...

  // TLB 模拟
  std::unordered_map<uint64_t, uint64_t> tlb_cache;  // VA -> PA
  size_t tlb_hits = 0;
  size_t tlb_misses = 0;
};
```

在 Mock 层添加对应接口：

```cpp
namespace cpu_io::virtual_memory {
  inline void FlushTLB(uint64_t va) {
    auto& core = test_env::TestEnvironmentState::GetInstance()
        .GetCurrentCoreEnv();
    core.tlb_cache.erase(va);
  }
}
```

### 添加性能计数器

```cpp
struct CoreEnvironment {
  struct PerfCounters {
    uint64_t instructions_executed = 0;
    uint64_t cache_misses = 0;
    uint64_t branch_mispredictions = 0;
  } perf_counters;
};
```

## 注意事项

1. **上下文注册**: 创建任务时需要调用 `RegisterTaskContext` 将任务上下文注册到环境层
2. **线程绑定**: 多核测试时每个工作线程都需要显式绑定到核心
3. **状态重置**: 每个测试结束后环境层会自动重置，无需手动清理
4. **线程安全**: 所有环境层接口都是线程安全的，可以在多线程环境中使用

## 文件清单

- `mocks/test_environment_state.hpp` - 环境层头文件
- `mocks/test_environment_state.cpp` - 环境层实现
- `mocks/cpu_io.h` - Mock 层硬件接口
- `mocks/arch_mock.cpp` - Mock 层架构函数
- `task/fixtures/task_test_harness.hpp` - 测试 Fixture 基类头文件
- `task/fixtures/task_test_harness.cpp` - 测试 Fixture 基类实现
- `task/example_environment_test.cpp` - 环境层功能测试示例
