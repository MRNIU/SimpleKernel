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

// 前向声明，防止循环依赖
struct TaskControlBlock;
struct CpuSchedData;

namespace test_env {

/**
 * @brief 单个核心的环境状态
 */
struct CoreEnvironment {
  // 核心标识
  size_t core_id = 0;

  // 中断使能状态
  bool interrupt_enabled = true;

  // 当前页表基地址
  uint64_t page_directory = 0;
  // 分页是否启用
  bool paging_enabled = false;

  // 上下文切换历史
  struct SwitchEvent {
    // 切换发生的时刻（从 sched_data->local_tick）
    uint64_t timestamp;
    // 源任务
    TaskControlBlock* from;
    // 目标任务
    TaskControlBlock* to;
    // 执行切换的核心
    size_t core_id;
  };
  std::vector<SwitchEvent> switch_history;

  // 关联的 std::thread ID
  std::thread::id thread_id;

  // 构造函数
  explicit CoreEnvironment(size_t id = 0) : core_id(id) {}
};

/**
 * @brief 测试环境状态
 *
 * 设计说明：
 * - 每个测试 Fixture 持有自己独立的 TestEnvironmentState 实例
 * - 通过线程局部指针 (thread_local) 让 Mock 层能访问当前测试的环境
 * - 这样既保证了测试隔离，又让 Mock 层无需显式传递环境参数
 *
 * 工作流程：
 * 1. 测试 Fixture 创建 TestEnvironmentState 实例（成员变量）
 * 2. SetUp() 中调用 SetCurrentThreadEnvironment() 设置线程局部指针
 * 3. Mock 层通过 GetCurrentThreadEnvironment() 获取当前环境
 * 4. TearDown() 中调用 ClearCurrentThreadEnvironment() 清理
 *
 * 示例：
 * @code
 * class MyTest : public ::testing::Test {
 *  protected:
 *   void SetUp() override {
 *     env_.InitializeCores(2);
 *     env_.SetCurrentThreadEnvironment();  // 关键：设置线程局部指针
 *     env_.BindThreadToCore(std::this_thread::get_id(), 0);
 *   }
 *   void TearDown() override {
 *     env_.ClearCurrentThreadEnvironment();  // 清理
 *   }
 *   TestEnvironmentState env_;  // 每个测试独立的环境
 * };
 * @endcode
 */
class TestEnvironmentState {
 public:
  TestEnvironmentState() = default;
  ~TestEnvironmentState() = default;

  TestEnvironmentState(const TestEnvironmentState&) = delete;
  TestEnvironmentState(TestEnvironmentState&&) = delete;
  auto operator=(const TestEnvironmentState&) -> TestEnvironmentState& = delete;
  auto operator=(TestEnvironmentState&&) -> TestEnvironmentState& = delete;

  /**
   * @brief 初始化指定数量的核心
   * @param num_cores 核心数量
   * @note 通常在 SetUp() 中调用
   */
  void InitializeCores(size_t num_cores);

  /**
   * @brief 重置所有核心状态（中断、页表、历史记录）
   * @note 不会改变核心数量，只重置状态
   */
  void ResetAllCores();

  /**
   * @brief 获取指定核心的环境
   * @param core_id 核心 ID
   * @return 核心环境引用
   * @throws std::out_of_range 如果 core_id 无效
   */
  auto GetCore(size_t core_id) -> CoreEnvironment&;

  /**
   * @brief 获取核心数量
   */
  auto GetCoreCount() const -> size_t;

  /**
   * @brief 将指定线程绑定到核心
   * @param tid 线程 ID
   * @param core_id 核心 ID
   * @note 用于多核测试，每个工作线程需要绑定到不同核心
   */
  void BindThreadToCore(std::thread::id tid, size_t core_id);

  /**
   * @brief 获取线程对应的核心 ID
   * @param tid 线程 ID
   * @return 核心 ID，如果未绑定则自动分配
   */
  auto GetCoreIdForThread(std::thread::id tid) -> size_t;

  /**
   * @brief 获取当前线程的核心环境
   * @return 当前线程对应的核心环境引用
   */
  auto GetCurrentCoreEnv() -> CoreEnvironment&;

  /**
   * @brief 注册任务上下文，建立上下文指针到任务的映射
   * @param context_ptr 任务的上下文指针（&task.task_context）
   * @param task 任务指针
   * @note switch_to 需要通过上下文指针找到对应任务
   */
  void RegisterTaskContext(void* context_ptr, TaskControlBlock* task);

  /**
   * @brief 注销任务上下文
   */
  void UnregisterTaskContext(void* context_ptr);

  /**
   * @brief 通过上下文指针查找任务
   * @return 任务指针，未找到返回 nullptr
   */
  auto FindTaskByContext(void* context_ptr) -> TaskControlBlock*;

  /**
   * @brief 打印所有核心的状态信息
   */
  void DumpAllCoreStates() const;

  /**
   * @brief 获取所有核心的切换历史，按时间戳排序
   * @return 切换事件列表
   */
  auto GetAllSwitchHistory() const -> std::vector<CoreEnvironment::SwitchEvent>;

  /**
   * @brief 清空所有核心的切换历史
   */
  void ClearSwitchHistory();

  /**
   * @brief 设置当前线程的环境实例指针
   * @note 必须在 SetUp() 中调用，让 Mock 层能访问这个环境
   *
   * 原理：设置一个 thread_local 指针指向 this
   * 效果：同一线程中，Mock 层可以通过 GetCurrentThreadEnvironment()
   * 获取这个环境
   */
  void SetCurrentThreadEnvironment();

  /**
   * @brief 清除当前线程的环境实例指针
   * @note 应在 TearDown() 中调用
   */
  void ClearCurrentThreadEnvironment();

  /**
   * @brief 获取当前线程的环境实例指针（供 Mock 层调用）
   * @return 环境指针，如果未设置则返回 nullptr
   *
   * 用法：Mock 层（cpu_io.h、arch.cpp）通过这个函数获取当前测试的环境
   * 例如：auto* env = TestEnvironmentState::GetCurrentThreadEnvironment();
   */
  static auto GetCurrentThreadEnvironment() -> TestEnvironmentState*;

 private:
  // 核心环境列表（每个核心一个）
  std::vector<CoreEnvironment> cores_;

  // 线程 ID 到核心 ID 的映射
  std::unordered_map<std::thread::id, size_t> thread_to_core_map_;

  // 上下文指针到任务的映射
  std::unordered_map<void*, TaskControlBlock*> context_to_task_map_;

  // 保护以上数据结构的互斥锁
  mutable std::mutex map_mutex_;

  // 下一个自动分配的核心 ID
  size_t next_core_id_ = 0;
};

}  // namespace test_env

#endif /* SIMPLEKERNEL_TESTS_UNIT_TEST_MOCKS_TEST_ENVIRONMENT_STATE_HPP_ */
