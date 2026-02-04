/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_TESTS_UNIT_TEST_MOCKS_TEST_ENVIRONMENT_STATE_HPP_
#define SIMPLEKERNEL_TESTS_UNIT_TEST_MOCKS_TEST_ENVIRONMENT_STATE_HPP_

#include <cstddef>
#include <cstdint>
#include <mutex>
#include <thread>
#include <unordered_map>
#include <vector>

struct TaskControlBlock;
struct CpuSchedData;

namespace test_env {

/**
 * @brief 单个核心的环境状态
 * 模拟真实硬件的 per-cpu 数据
 */
struct CoreEnvironment {
  // 核心标识
  size_t core_id = 0;

  // 中断状态
  bool interrupt_enabled = true;
  // 中断嵌套深度
  int interrupt_nest_level = 0;

  // 当前页表基地址
  uint64_t page_directory = 0;
  // 页表状态
  bool paging_enabled = false;

  // 线程状态
  TaskControlBlock* current_thread = nullptr;
  TaskControlBlock* idle_thread = nullptr;

  // 调度数据
  CpuSchedData* sched_data = nullptr;

  // 上下文切换历史 (用于验证)
  struct SwitchEvent {
    uint64_t timestamp;
    TaskControlBlock* from;
    TaskControlBlock* to;
    size_t core_id;
  };
  std::vector<SwitchEvent> switch_history;

  // 性能统计
  uint64_t local_tick = 0;
  uint64_t total_switches = 0;

  // 线程同步
  std::thread::id thread_id;

  // 构造函数
  explicit CoreEnvironment(size_t id = 0) : core_id(id) {}
};

/**
 * @brief 全局测试环境状态
 * 管理所有核心的环境
 */
class TestEnvironmentState {
 public:
  // 获取单例
  static auto GetInstance() -> TestEnvironmentState&;

  // 核心管理
  void InitializeCores(size_t num_cores);
  void ResetAllCores();
  auto GetCore(size_t core_id) -> CoreEnvironment&;
  auto GetCoreCount() const -> size_t;

  // 线程到核心的映射
  void BindThreadToCore(std::thread::id tid, size_t core_id);
  auto GetCoreIdForThread(std::thread::id tid) -> size_t;
  auto GetCurrentCoreEnv() -> CoreEnvironment&;

  // 上下文到任务的映射 (用于 switch_to)
  void RegisterTaskContext(void* context_ptr, TaskControlBlock* task);
  void UnregisterTaskContext(void* context_ptr);
  auto FindTaskByContext(void* context_ptr) -> TaskControlBlock*;

  // 调试与验证
  void DumpAllCoreStates() const;
  auto GetAllSwitchHistory() const -> std::vector<CoreEnvironment::SwitchEvent>;
  void ClearSwitchHistory();

 private:
  TestEnvironmentState() = default;

  std::vector<CoreEnvironment> cores_;
  std::unordered_map<std::thread::id, size_t> thread_to_core_map_;
  std::unordered_map<void*, TaskControlBlock*> context_to_task_map_;
  mutable std::mutex map_mutex_;
  size_t next_core_id_ = 0;
};

}  // namespace test_env

#endif /* SIMPLEKERNEL_TESTS_UNIT_TEST_MOCKS_TEST_ENVIRONMENT_STATE_HPP_ */
