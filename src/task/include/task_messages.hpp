/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Task FSM message IDs and router IDs — single source of truth
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_

#include <etl/message.h>

/// @brief Task FSM message IDs
namespace task_msg_id {
inline constexpr etl::message_id_t kSchedule = 1;
inline constexpr etl::message_id_t kYield = 2;
inline constexpr etl::message_id_t kSleep = 3;
inline constexpr etl::message_id_t kBlock = 4;
inline constexpr etl::message_id_t kWakeup = 5;
inline constexpr etl::message_id_t kExit = 6;
inline constexpr etl::message_id_t kReap = 7;
}  // namespace task_msg_id

/// @brief Message router IDs
namespace router_id {
constexpr etl::message_router_id_t kTimerHandler = 0;
constexpr etl::message_router_id_t kTaskFsm = 1;
constexpr etl::message_router_id_t kVirtioBlk = 2;
constexpr etl::message_router_id_t kVirtioNet = 3;
}  // namespace router_id

/// @brief Task FSM message structs (empty payload, used as events)
struct MsgSchedule : public etl::message<task_msg_id::kSchedule> {};
struct MsgYield : public etl::message<task_msg_id::kYield> {};
struct MsgWakeup : public etl::message<task_msg_id::kWakeup> {};
struct MsgReap : public etl::message<task_msg_id::kReap> {};

/// @brief Sleep message with wake tick payload
struct MsgSleep : public etl::message<task_msg_id::kSleep> {
  uint64_t wake_tick;
  explicit MsgSleep(uint64_t tick) : wake_tick(tick) {}
};

/// @brief Block message with resource payload
/// @note Forward-declared ResourceId to avoid circular include
struct MsgBlock : public etl::message<task_msg_id::kBlock> {
  uint64_t resource_data;
  explicit MsgBlock(uint64_t data) : resource_data(data) {}
};

/// @brief Exit message with exit code and parent flag
struct MsgExit : public etl::message<task_msg_id::kExit> {
  int exit_code;
  bool has_parent;
  MsgExit(int code, bool parent) : exit_code(code), has_parent(parent) {}
};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
