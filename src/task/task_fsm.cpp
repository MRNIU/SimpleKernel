/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task_fsm.hpp"

#include "kernel_log.hpp"

auto StateUnInit::on_event(const MsgSchedule& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateUnInit::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: UnInit received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

auto StateReady::on_event(const MsgSchedule& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kRunning;
}

auto StateReady::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Ready received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

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

auto StateSleeping::on_event(const MsgWakeup& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateSleeping::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Sleeping received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

auto StateBlocked::on_event(const MsgWakeup& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateBlocked::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Blocked received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

auto StateExited::on_event(const MsgReap& /*msg*/) -> etl::fsm_state_id_t {
  return STATE_ID;
}

auto StateExited::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Exited received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

auto StateZombie::on_event(const MsgReap& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kExited;
}

auto StateZombie::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: Zombie received unexpected message id=%d\n",
             static_cast<int>(msg.get_message_id()));
  return STATE_ID;
}

TaskFsm::TaskFsm() : fsm_(router_id::kTaskFsm) {
  state_list_[0] = &state_uninit_;
  state_list_[1] = &state_ready_;
  state_list_[2] = &state_running_;
  state_list_[3] = &state_sleeping_;
  state_list_[4] = &state_blocked_;
  state_list_[5] = &state_exited_;
  state_list_[6] = &state_zombie_;
  fsm_.set_states(state_list_, 7);
}

void TaskFsm::Start() { fsm_.start(); }

void TaskFsm::Receive(const etl::imessage& msg) { fsm_.receive(msg); }

auto TaskFsm::GetStateId() const -> etl::fsm_state_id_t {
  return fsm_.get_state_id();
}
