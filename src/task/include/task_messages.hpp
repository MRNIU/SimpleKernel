/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_

#include <etl/message.h>

#include "resource_id.hpp"

/// Task FSM 消息 ID
namespace task_msg_id {
static constexpr etl::message_id_t kSchedule = 1;
static constexpr etl::message_id_t kYield = 2;
static constexpr etl::message_id_t kSleep = 3;
static constexpr etl::message_id_t kBlock = 4;
static constexpr etl::message_id_t kWakeup = 5;
static constexpr etl::message_id_t kExit = 6;
static constexpr etl::message_id_t kReap = 7;
}  // namespace task_msg_id

/// 消息路由 ID
namespace router_id {
static constexpr etl::message_router_id_t kTimerHandler = 0;
static constexpr etl::message_router_id_t kTaskFsm = 1;
static constexpr etl::message_router_id_t kVirtioBlk = 2;
static constexpr etl::message_router_id_t kVirtioNet = 3;
}  // namespace router_id

/// Task FSM 消息结构体（无负载，用作事件）
struct MsgSchedule : public etl::message<task_msg_id::kSchedule> {};
struct MsgYield : public etl::message<task_msg_id::kYield> {};
struct MsgWakeup : public etl::message<task_msg_id::kWakeup> {};
struct MsgReap : public etl::message<task_msg_id::kReap> {};

/// 睡眠消息，携带唤醒时钟
struct MsgSleep : public etl::message<task_msg_id::kSleep> {
  uint64_t wake_tick;
  explicit MsgSleep(uint64_t tick) : wake_tick(tick) {}
};

/// 阻塞消息，携带资源 ID
struct MsgBlock : public etl::message<task_msg_id::kBlock> {
  ResourceId resource_id;
  explicit MsgBlock(ResourceId id) : resource_id(id) {}
};

/// 退出消息，携带退出码与父任务标志
struct MsgExit : public etl::message<task_msg_id::kExit> {
  int exit_code;
  bool has_parent;
  MsgExit(int code, bool parent) : exit_code(code), has_parent(parent) {}
};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
