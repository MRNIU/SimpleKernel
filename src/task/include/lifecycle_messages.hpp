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

/// @brief 线程创建消息
struct ThreadCreateMsg : public etl::message<lifecycle_msg_id::kThreadCreate> {
  size_t pid{0};
  explicit ThreadCreateMsg(size_t p) : pid(p) {}
};

/// @brief 线程退出消息
struct ThreadExitMsg : public etl::message<lifecycle_msg_id::kThreadExit> {
  size_t pid{0};
  int exit_code{0};
  ThreadExitMsg(size_t p, int code) : pid(p), exit_code(code) {}
};
