/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Task FSM implementation — etl::fsm state transition bodies
 */

#include "task_fsm.hpp"

#include "kernel_log.hpp"

// ─── StateUnInit ─────────────────────────────────────────────────────────────

auto StateUnInit::on_event(const MsgSchedule& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateUnInit::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: UnInit received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

// ─── StateReady ──────────────────────────────────────────────────────────────

auto StateReady::on_event(const MsgSchedule& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kRunning;
}

auto StateReady::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Ready received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

// ─── StateRunning ────────────────────────────────────────────────────────────

auto StateRunning::on_event(const MsgYield& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateRunning::on_event(const MsgSleep& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kSleeping;
}

auto StateRunning::on_event(const MsgBlock& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kBlocked;
}

auto StateRunning::on_event(const MsgExit& msg) -> etl::fsm_state_id_t {
  if (msg.has_parent) {
    return TaskStatusId::kZombie;
  }
  return TaskStatusId::kExited;
}

auto StateRunning::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Running received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

// ─── StateSleeping ───────────────────────────────────────────────────────────

auto StateSleeping::on_event(const MsgWakeup& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateSleeping::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Sleeping received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

// ─── StateBlocked ────────────────────────────────────────────────────────────

auto StateBlocked::on_event(const MsgWakeup& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateBlocked::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Blocked received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

// ─── StateExited ─────────────────────────────────────────────────────────────

auto StateExited::on_event(const MsgReap& /*msg*/) -> etl::fsm_state_id_t {
  return STATE_ID;
}

auto StateExited::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Exited received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

// ─── StateZombie ─────────────────────────────────────────────────────────────

auto StateZombie::on_event(const MsgReap& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kExited;
}

auto StateZombie::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Zombie received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

// ─── TaskFsm ─────────────────────────────────────────────────────────────────

TaskFsm::TaskFsm() : fsm_(router_id::kTaskFsm) {
  etl::ifsm_state* state_list[] = {
      &state_uninit_,  &state_ready_,  &state_running_, &state_sleeping_,
      &state_blocked_, &state_exited_, &state_zombie_,
  };
  fsm_.set_states(state_list, 7);
}

void TaskFsm::Start() { fsm_.start(); }

void TaskFsm::Receive(const etl::imessage& msg) { fsm_.receive(msg); }

auto TaskFsm::GetStateId() const -> etl::fsm_state_id_t {
  return fsm_.get_state_id();
}
