# Task 模块 Unit Test 设计（精简版）

## 1. 目标
1) 支持单核/多核调度测试。 2) 每个测试独立、无全局污染。 3) 处理死循环 `while(1) cpu_io::Pause()`。 4) 使用 `std::thread` 模拟核心。 5) 最小化 Mock，尽量复用现有 `TaskManager/PerCpu/Scheduler`。

## 2. 架构
三层结构，职责清晰：
1) `TaskTestHarness`（Fixture）：统一 SetUp/TearDown，重置全局状态。
2) `TaskTestEnvironment`：管理多核线程、任务创建、等待条件。
3) `SchedulingWorker`：每核一个线程，循环调用 `Schedule()`/`TickUpdate()`。

依赖现有：`TaskManager`、`PerCpu`、`FifoScheduler`、`RoundRobinScheduler`、`IdleScheduler`。 Mock 仅覆盖架构相关接口与核心 ID 映射。

## 3. 关键机制
### 3.1 测试隔离
`TaskTestHarness::SetUp()` 完成：
- 重置 `TaskManager`（新增 `ResetForTesting()`）
- 清理 PID 分配器与全局任务表
- 重置 `PerCpu` 数组
- 重新初始化测试环境

### 3.2 多核支持与亲和性
- `MockCoreManager` 管理线程→核心 ID 映射。 `cpu_io::GetCurrentCoreId()` 从这里取值。
- `TaskTestEnvironment::AddTask()`：
  - 若 `task->cpu_affinity` 指定，放入允许的核心（可选择负载最低核心）。
  - 若未指定，按各核心队列长度/负载均衡选择。

### 3.3 `switch_to` 处理（架构无关）
单测中不依赖汇编实现，提供“软切换”测试桩：
- 记录切换事件（from/to/时间戳）。
- 更新 `per_cpu::running_task`。
- 不执行真实栈切换。
调度逻辑仍由 `TaskManager::Schedule()` 真实执行，因此可验证调度策略。

### 3.4 超时保护（避免死循环）
`TimeoutGuard` 在 4 秒内监控测试：
- 超时仅设置原子标志并停止调度线程（不直接调用 GTest 失败）。
- 主线程检查 `IsTimeout()` 并决定失败。

## 4. 文件结构（最小集）
`tests/unit_test/task/`
- `fixtures/`：`task_test_harness.*`, `task_test_environment.*`, `scheduling_worker.*`, `mock_core_manager.*`, `timeout_guard.*`
- `unit_tests/`：调度器、生命周期、多核、阻塞/唤醒等测试

## 5. 必要的最小改动
1) `TaskManager::ResetForTesting()`：清空调度器、任务表、统计信息。
2) `TaskControlBlock` 可选提供便捷构造（便于测试创建任务）。
3) 单测目标链接测试桩版 `switch_to`（不走架构汇编）。

## 6. 典型测试覆盖
- 单核：RR/FIFO 时间片与顺序。
- 多核：负载均衡与亲和性。
- 状态：Ready→Running→Blocked/Sleeping→Ready。
- 异常：超时处理、死锁/无限循环检测。

## 7. 为什么这样做
- 调度逻辑真实运行，Mock 最少。
- 代码量小、职责清晰、维护成本低。
- 多核与异常场景可复现且可控。
