/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "test_environment_state.hpp"

#include <algorithm>
#include <iostream>
#include <stdexcept>

#include "task_control_block.hpp"

namespace test_env {

auto TestEnvironmentState::GetInstance() -> TestEnvironmentState& {
  static TestEnvironmentState instance;
  return instance;
}

void TestEnvironmentState::InitializeCores(size_t num_cores) {
  std::lock_guard<std::mutex> lock(map_mutex_);
  cores_.clear();
  cores_.reserve(num_cores);
  for (size_t i = 0; i < num_cores; ++i) {
    cores_.emplace_back(i);
  }
}

void TestEnvironmentState::ResetAllCores() {
  std::lock_guard<std::mutex> lock(map_mutex_);
  for (auto& core : cores_) {
    core.interrupt_enabled = true;
    core.interrupt_nest_level = 0;
    core.page_directory = 0;
    core.paging_enabled = false;
    core.current_thread = nullptr;
    core.idle_thread = nullptr;
    core.sched_data = nullptr;
    core.switch_history.clear();
    core.local_tick = 0;
    core.total_switches = 0;
  }
  thread_to_core_map_.clear();
  context_to_task_map_.clear();
  next_core_id_ = 0;
}

auto TestEnvironmentState::GetCore(size_t core_id) -> CoreEnvironment& {
  std::lock_guard<std::mutex> lock(map_mutex_);
  if (core_id >= cores_.size()) {
    throw std::out_of_range("Invalid core_id: " + std::to_string(core_id));
  }
  return cores_[core_id];
}

auto TestEnvironmentState::GetCoreCount() const -> size_t {
  std::lock_guard<std::mutex> lock(map_mutex_);
  return cores_.size();
}

void TestEnvironmentState::BindThreadToCore(std::thread::id tid,
                                            size_t core_id) {
  std::lock_guard<std::mutex> lock(map_mutex_);
  if (core_id >= cores_.size()) {
    throw std::out_of_range("Invalid core_id: " + std::to_string(core_id));
  }
  thread_to_core_map_[tid] = core_id;
  cores_[core_id].thread_id = tid;
}

auto TestEnvironmentState::GetCoreIdForThread(std::thread::id tid) -> size_t {
  std::lock_guard<std::mutex> lock(map_mutex_);
  auto it = thread_to_core_map_.find(tid);
  if (it != thread_to_core_map_.end()) {
    return it->second;
  }

  // 自动分配一个核心 ID (兼容旧行为)
  if (next_core_id_ >= cores_.size()) {
    // 如果核心未初始化，默认创建单核环境
    if (cores_.empty()) {
      cores_.emplace_back(0);
    }
    next_core_id_ = 0;
  }

  size_t new_id = next_core_id_;
  thread_to_core_map_[tid] = new_id;
  cores_[new_id].thread_id = tid;
  next_core_id_ = (next_core_id_ + 1) % cores_.size();
  return new_id;
}

auto TestEnvironmentState::GetCurrentCoreEnv() -> CoreEnvironment& {
  auto tid = std::this_thread::get_id();
  size_t core_id = GetCoreIdForThread(tid);
  std::lock_guard<std::mutex> lock(map_mutex_);
  return cores_[core_id];
}

void TestEnvironmentState::RegisterTaskContext(void* context_ptr,
                                               TaskControlBlock* task) {
  std::lock_guard<std::mutex> lock(map_mutex_);
  context_to_task_map_[context_ptr] = task;
}

void TestEnvironmentState::UnregisterTaskContext(void* context_ptr) {
  std::lock_guard<std::mutex> lock(map_mutex_);
  context_to_task_map_.erase(context_ptr);
}

auto TestEnvironmentState::FindTaskByContext(void* context_ptr)
    -> TaskControlBlock* {
  std::lock_guard<std::mutex> lock(map_mutex_);
  auto it = context_to_task_map_.find(context_ptr);
  if (it != context_to_task_map_.end()) {
    return it->second;
  }
  return nullptr;
}

void TestEnvironmentState::DumpAllCoreStates() const {
  std::lock_guard<std::mutex> lock(map_mutex_);
  std::cout << "\n=== Test Environment State Dump ===" << std::endl;
  std::cout << "Total cores: " << cores_.size() << std::endl;

  for (const auto& core : cores_) {
    std::cout << "\nCore " << core.core_id << ":" << std::endl;
    std::cout << "  Interrupt enabled: " << core.interrupt_enabled << std::endl;
    std::cout << "  Interrupt nest level: " << core.interrupt_nest_level
              << std::endl;
    std::cout << "  Page directory: 0x" << std::hex << core.page_directory
              << std::dec << std::endl;
    std::cout << "  Paging enabled: " << core.paging_enabled << std::endl;
    std::cout << "  Current thread: " << core.current_thread << std::endl;
    std::cout << "  Total switches: " << core.total_switches << std::endl;
    std::cout << "  Local tick: " << core.local_tick << std::endl;
    std::cout << "  Switch history size: " << core.switch_history.size()
              << std::endl;
  }
  std::cout << "==================================\n" << std::endl;
}

auto TestEnvironmentState::GetAllSwitchHistory() const
    -> std::vector<CoreEnvironment::SwitchEvent> {
  std::lock_guard<std::mutex> lock(map_mutex_);
  std::vector<CoreEnvironment::SwitchEvent> all_events;

  for (const auto& core : cores_) {
    all_events.insert(all_events.end(), core.switch_history.begin(),
                      core.switch_history.end());
  }

  // 按时间戳排序
  std::sort(
      all_events.begin(), all_events.end(),
      [](const auto& a, const auto& b) { return a.timestamp < b.timestamp; });

  return all_events;
}

void TestEnvironmentState::ClearSwitchHistory() {
  std::lock_guard<std::mutex> lock(map_mutex_);
  for (auto& core : cores_) {
    core.switch_history.clear();
  }
}

}  // namespace test_env
