/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task_manager.hpp"

Pid TaskManager::Clone(uint64_t flags, void* user_stack, int* parent_tid,
                       int* child_tid, void* tls,
                       cpu_io::TrapContext& parent_context) {
  /// @todo 实现 clone 功能，需要考虑对不容架构的支持
  (void)flags;
  (void)user_stack;
  (void)parent_tid;
  (void)child_tid;
  (void)tls;
  (void)parent_context;
  return -1;
}
