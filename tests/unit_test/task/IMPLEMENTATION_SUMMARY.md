# Task 单元测试环境层实现总结

## 实现概览

已成功实现 Task 模块单元测试的三层架构设计：

```
测试层 (Test Layer)
    ↓ 调用 Mock API
Mock 层 (Mock Layer) - cpu_io.h, arch_mock.cpp
    ↓ 操作环境状态
环境层 (Environment Layer) - TestEnvironmentState, CoreEnvironment
```

## 已创建的文件

### 核心实现文件

1. **环境层**
   - `tests/unit_test/mocks/test_environment_state.hpp` - 环境层头文件
   - `tests/unit_test/mocks/test_environment_state.cpp` - 环境层实现

2. **Mock 层（已修改）**
   - `tests/unit_test/mocks/cpu_io.h` - 更新为使用环境层
   - `tests/unit_test/mocks/arch_mock.cpp` - 增强 switch_to 实现

3. **测试框架**
   - `tests/unit_test/task/fixtures/task_test_harness.hpp` - 测试基类头文件
   - `tests/unit_test/task/fixtures/task_test_harness.cpp` - 测试基类实现

4. **示例与文档**
   - `tests/unit_test/task/example_environment_test.cpp` - 环境层功能测试
   - `tests/unit_test/task/task_scheduling_example_test.cpp` - 任务调度测试示例
   - `tests/unit_test/task/README_ENVIRONMENT_LAYER.md` - 详细使用文档

5. **构建配置**
   - `tests/unit_test/CMakeLists.txt` - 已更新包含新文件

## 核心功能

### 1. CoreEnvironment（单核环境状态）

模拟单个 CPU 核心的所有关键状态：
- ✅ 核心 ID
- ✅ 中断状态（使能/禁用、嵌套深度）
- ✅ 页表状态（页表基地址、分页使能）
- ✅ 线程状态（当前线程、空闲线程）
- ✅ 调度数据指针
- ✅ 上下文切换历史记录
- ✅ 性能统计（tick 计数、切换次数）
- ✅ 线程 ID 映射

### 2. TestEnvironmentState（全局环境管理）

提供统一的环境管理接口：
- ✅ 多核初始化与重置
- ✅ 线程到核心的映射
- ✅ 上下文到任务的映射（用于 switch_to）
- ✅ 调试状态转储
- ✅ 切换历史聚合与清理
- ✅ 线程安全（互斥锁保护）

### 3. Mock 层集成

所有硬件接口都通过环境层实现：
- ✅ `GetCurrentCoreId()` - 查询当前核心 ID
- ✅ `EnableInterrupt() / DisableInterrupt()` - 中断控制
- ✅ `GetInterruptStatus()` - 查询中断状态
- ✅ `SetPageDirectory()` - 设置页表
- ✅ `GetPageDirectory()` - 查询页表
- ✅ `EnablePage()` - 启用分页
- ✅ `switch_to()` - 记录上下文切换

### 4. 测试框架

提供统一的测试基类：
- ✅ `TaskTestHarness` - 自动管理环境生命周期
- ✅ 测试隔离（每个测试独立重置环境）
- ✅ 多核支持（可配置核心数量）
- ✅ 主线程自动绑定到 core 0

## 使用流程

### 1. 编写测试类

```cpp
class MyTaskTest : public TaskTestHarness {
protected:
  void SetUp() override {
    SetNumCores(2);  // 配置核心数量
    TaskTestHarness::SetUp();
  }
};
```

### 2. 注册任务上下文

```cpp
TEST_F(MyTaskTest, TaskSwitching) {
  auto& env = GetEnvironmentState();

  TaskControlBlock task1, task2;
  env.RegisterTaskContext(&task1.task_context, &task1);
  env.RegisterTaskContext(&task2.task_context, &task2);

  // ... 执行测试 ...

  env.UnregisterTaskContext(&task1.task_context);
  env.UnregisterTaskContext(&task2.task_context);
}
```

### 3. 验证调度行为

```cpp
// 执行任务切换
extern "C" void switch_to(cpu_io::CalleeSavedContext*,
                         cpu_io::CalleeSavedContext*);
switch_to(&task1.task_context, &task2.task_context);

// 验证切换历史
auto history = env.GetAllSwitchHistory();
EXPECT_EQ(history[0].from, &task1);
EXPECT_EQ(history[0].to, &task2);
```

## 已验证的功能

通过示例测试验证了以下功能：

### 基础功能测试（example_environment_test.cpp）
- ✅ 核心初始化
- ✅ 中断状态控制
- ✅ 页表操作
- ✅ CoreId 查询
- ✅ 状态转储
- ✅ 切换历史追踪

### 调度功能测试（task_scheduling_example_test.cpp）
- ✅ 任务上下文注册与注销
- ✅ switch_to 记录切换事件
- ✅ 多次切换的历史记录
- ✅ 中断状态在切换时的行为
- ✅ 页表在任务间切换
- ✅ 切换历史清空

## 设计优势

### 1. 职责清晰
- **环境层**: 纯数据容器，存储模拟状态
- **Mock 层**: 提供硬件接口，操作环境层
- **测试层**: 编写测试逻辑，验证行为

### 2. 可观测性强
- 所有硬件状态可随时查询
- 完整的上下文切换历史
- 性能统计信息（tick、切换次数）

### 3. 易于扩展
需要添加新的硬件特性时，只需：
1. 在 `CoreEnvironment` 中添加字段
2. 在 Mock 层添加对应接口
3. 测试层自动可用

示例扩展（TLB 模拟）：
```cpp
// 1. 扩展 CoreEnvironment
struct CoreEnvironment {
  std::unordered_map<uint64_t, uint64_t> tlb_cache;
  size_t tlb_hits = 0;
  size_t tlb_misses = 0;
};

// 2. 添加 Mock 接口
namespace cpu_io::virtual_memory {
  inline void FlushTLB(uint64_t va) {
    auto& core = test_env::TestEnvironmentState::GetInstance()
        .GetCurrentCoreEnv();
    core.tlb_cache.erase(va);
  }
}

// 3. 测试层直接使用
TEST_F(MyTest, TLBBehavior) {
  cpu_io::virtual_memory::FlushTLB(0x1000);
  auto& core = GetEnvironmentState().GetCurrentCoreEnv();
  EXPECT_TRUE(core.tlb_cache.empty());
}
```

### 4. 线程安全
- 所有环境层接口使用互斥锁保护
- 支持真实的多线程测试
- 自动核心映射机制

### 5. 测试隔离
- 每个测试自动重置环境
- 无全局状态污染
- 测试间完全独立

## 下一步工作

### 建议优先实现的功能

1. **TaskManager 的 ResetForTesting 方法**
   ```cpp
   class TaskManager {
   public:
     void ResetForTesting() {
       // 清空调度器队列
       // 重置 PID 分配器
       // 清理任务表
     }
   };
   ```

2. **多核工作线程框架**
   ```cpp
   class SchedulingWorker {
   public:
     SchedulingWorker(size_t core_id);
     void Start();
     void Stop();
   private:
     void WorkerLoop();
   };
   ```

3. **超时保护机制**
   ```cpp
   class TimeoutGuard {
   public:
     TimeoutGuard(std::chrono::seconds timeout);
     bool IsTimeout() const;
   };
   ```

4. **任务创建辅助函数**
   ```cpp
   auto CreateTestTask(const char* name, ThreadEntry func, void* arg)
       -> TaskControlBlock*;
   ```

### 可以开始编写的测试

有了这个环境层，现在可以编写：
- ✅ 调度器单元测试（FIFO、RR、CFS）
- ✅ 任务状态转换测试
- ✅ 多核负载均衡测试
- ✅ CPU 亲和性测试
- ✅ 阻塞/唤醒测试
- ✅ 时间片管理测试

## 构建与运行

```bash
# 1. 配置构建（选择架构，以 riscv64 为例）
cmake --preset build_riscv64

# 2. 编译单元测试
cd build_riscv64 && make unit-test

# 3. 运行测试
./tests/unit-test

# 4. 运行特定测试
./tests/unit-test --gtest_filter="EnvironmentLayerTest.*"
./tests/unit-test --gtest_filter="TaskSchedulingExample.*"
```

## 文档参考

- [环境层使用指南](tests/unit_test/task/README_ENVIRONMENT_LAYER.md) - 详细的 API 文档和使用示例
- [Task 单元测试设计](doc/task_unit_test_new_design.md) - 整体设计思路

## 技术细节

### 线程到核心的映射

使用 `std::thread::id` 作为 key，自动或手动绑定：

```cpp
// 自动绑定（首次调用时）
size_t core_id = cpu_io::GetCurrentCoreId();  // 自动分配

// 手动绑定（多核测试时推荐）
env.BindThreadToCore(std::this_thread::get_id(), 2);
```

### 上下文到任务的映射

用于 `switch_to` 函数找到对应的任务：

```cpp
// 注册
env.RegisterTaskContext(&task.task_context, &task);

// 查找
auto* task = env.FindTaskByContext(context_ptr);

// 注销
env.UnregisterTaskContext(&task.task_context);
```

### 切换事件记录

每次调用 `switch_to` 都会自动记录：

```cpp
struct SwitchEvent {
  uint64_t timestamp;        // 切换时的 tick
  TaskControlBlock* from;    // 源任务
  TaskControlBlock* to;      // 目标任务
  size_t core_id;           // 执行切换的核心
};
```

## 总结

✅ **环境层实现完成** - 提供完整的硬件状态模拟
✅ **Mock 层集成完成** - 所有接口使用环境层
✅ **测试框架就绪** - TaskTestHarness 提供统一基类
✅ **示例测试通过** - 验证了核心功能
✅ **文档完善** - 详细的使用指南和示例

现在可以基于这个环境层编写完整的 Task 模块单元测试了！
