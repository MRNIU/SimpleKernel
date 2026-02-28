/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Task FSM — etl::fsm-based state machine for task lifecycle
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_

#include <etl/fsm.h>

#include "task_messages.hpp"

// Forward declaration
struct TaskControlBlock;

/// @brief Task status IDs — used as etl::fsm state IDs
/// @note These MUST match the old TaskStatus enum values for compatibility
enum TaskStatusId : uint8_t {
  kUnInit = 0,
  kReady = 1,
  kRunning = 2,
  kSleeping = 3,
  kBlocked = 4,
  kExited = 5,
  kZombie = 6,
};

// Forward-declare all states so they can reference each other in transition
// maps
class StateUnInit;
class StateReady;
class StateRunning;
class StateSleeping;
class StateBlocked;
class StateExited;
class StateZombie;

// ─── State: UnInit ───────────────────────────────────────────────────
class StateUnInit : public etl::fsm_state<etl::fsm, StateUnInit,
                                          TaskStatusId::kUnInit, MsgSchedule> {
 public:
  auto on_event(const MsgSchedule& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Ready ────────────────────────────────────────────────────
class StateReady : public etl::fsm_state<etl::fsm, StateReady,
                                         TaskStatusId::kReady, MsgSchedule> {
 public:
  auto on_event(const MsgSchedule& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Running ──────────────────────────────────────────────────
class StateRunning
    : public etl::fsm_state<etl::fsm, StateRunning, TaskStatusId::kRunning,
                            MsgYield, MsgSleep, MsgBlock, MsgExit> {
 public:
  auto on_event(const MsgYield& msg) -> etl::fsm_state_id_t;
  auto on_event(const MsgSleep& msg) -> etl::fsm_state_id_t;
  auto on_event(const MsgBlock& msg) -> etl::fsm_state_id_t;
  auto on_event(const MsgExit& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Sleeping ─────────────────────────────────────────────────
class StateSleeping
    : public etl::fsm_state<etl::fsm, StateSleeping, TaskStatusId::kSleeping,
                            MsgWakeup> {
 public:
  auto on_event(const MsgWakeup& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Blocked ──────────────────────────────────────────────────
class StateBlocked : public etl::fsm_state<etl::fsm, StateBlocked,
                                           TaskStatusId::kBlocked, MsgWakeup> {
 public:
  auto on_event(const MsgWakeup& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Exited ───────────────────────────────────────────────────
class StateExited : public etl::fsm_state<etl::fsm, StateExited,
                                          TaskStatusId::kExited, MsgReap> {
 public:
  auto on_event(const MsgReap& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Zombie ───────────────────────────────────────────────────
class StateZombie : public etl::fsm_state<etl::fsm, StateZombie,
                                          TaskStatusId::kZombie, MsgReap> {
 public:
  auto on_event(const MsgReap& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── TaskFsm ─────────────────────────────────────────────────────────

/// @brief Per-task FSM instance. Each TaskControlBlock owns one.
class TaskFsm {
 public:
  TaskFsm();

  /// @brief Start the FSM (call after TCB is fully constructed)
  void Start();

  /// @brief Send a message to the FSM
  void Receive(const etl::imessage& msg);

  /// @brief Get the current state ID
  auto GetStateId() const -> etl::fsm_state_id_t;

 private:
  StateUnInit state_uninit_;
  StateReady state_ready_;
  StateRunning state_running_;
  StateSleeping state_sleeping_;
  StateBlocked state_blocked_;
  StateExited state_exited_;
  StateZombie state_zombie_;

  etl::fsm fsm_;
};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_
