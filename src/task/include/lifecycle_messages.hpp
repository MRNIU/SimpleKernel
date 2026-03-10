/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#pragma once

#include <etl/message.h>

#include <cstddef>

namespace lifecycle_msg_id {
inline constexpr etl::message_id_t kThreadCreate = 100;
inline constexpr etl::message_id_t kThreadExit = 101;
}  // namespace lifecycle_msg_id

struct ThreadCreateMsg : public etl::message<lifecycle_msg_id::kThreadCreate> {
  size_t pid;
  explicit ThreadCreateMsg(size_t p) : pid(p) {}
};

struct ThreadExitMsg : public etl::message<lifecycle_msg_id::kThreadExit> {
  size_t pid;
  int exit_code;
  ThreadExitMsg(size_t p, int code) : pid(p), exit_code(code) {}
};
