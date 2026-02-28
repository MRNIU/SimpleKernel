/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_

#include <etl/fsm.h>

#include "kernel_log.hpp"
#include "task_messages.hpp"

/// 任务状态 ID — 用作 etl::fsm 的状态 ID
enum TaskStatusId : uint8_t {
  kUnInit = 0,
  kReady = 1,
  kRunning = 2,
  kSleeping = 3,
  kBlocked = 4,
  kExited = 5,
  kZombie = 6,
};

// 前向声明所有状态类，以便在转换表中相互引用
struct StateUnInit;
struct StateReady;
struct StateRunning;
struct StateSleeping;
struct StateBlocked;
struct StateExited;
struct StateZombie;

/// 状态：UnInit — 任务尚未初始化
struct StateUnInit : public etl::fsm_state<etl::fsm, StateUnInit,
                                           TaskStatusId::kUnInit, MsgSchedule> {
  auto on_event(const MsgSchedule&) -> etl::fsm_state_id_t {
    return TaskStatusId::kReady;
  }
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t {
    klog::Warn("TaskFsm: UnInit received unexpected message id=%d\n",
               static_cast<int>(msg.get_message_id()));
    return STATE_ID;
  }
};

/// 状态：Ready — 任务已就绪，等待调度
struct StateReady : public etl::fsm_state<etl::fsm, StateReady,
                                          TaskStatusId::kReady, MsgSchedule> {
  auto on_event(const MsgSchedule&) -> etl::fsm_state_id_t {
    return TaskStatusId::kRunning;
  }
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t {
    klog::Warn("TaskFsm: Ready received unexpected message id=%d\n",
               static_cast<int>(msg.get_message_id()));
    return STATE_ID;
  }
};

/// 状态：Running — 任务正在执行
struct StateRunning
    : public etl::fsm_state<etl::fsm, StateRunning, TaskStatusId::kRunning,
                            MsgYield, MsgSleep, MsgBlock, MsgExit> {
  auto on_event(const MsgYield&) -> etl::fsm_state_id_t {
    return TaskStatusId::kReady;
  }
  auto on_event(const MsgSleep&) -> etl::fsm_state_id_t {
    return TaskStatusId::kSleeping;
  }
  auto on_event(const MsgBlock&) -> etl::fsm_state_id_t {
    return TaskStatusId::kBlocked;
  }
  auto on_event(const MsgExit& msg) -> etl::fsm_state_id_t {
    if (msg.has_parent) {
      return TaskStatusId::kZombie;
    }
    return TaskStatusId::kExited;
  }
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t {
    klog::Warn("TaskFsm: Running received unexpected message id=%d\n",
               static_cast<int>(msg.get_message_id()));
    return STATE_ID;
  }
};

/// 状态：Sleeping — 任务已挂起，等待唤醒时钟
struct StateSleeping
    : public etl::fsm_state<etl::fsm, StateSleeping, TaskStatusId::kSleeping,
                            MsgWakeup> {
  auto on_event(const MsgWakeup&) -> etl::fsm_state_id_t {
    return TaskStatusId::kReady;
  }
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t {
    klog::Warn("TaskFsm: Sleeping received unexpected message id=%d\n",
               static_cast<int>(msg.get_message_id()));
    return STATE_ID;
  }
};

/// 状态：Blocked — 任务阻塞，等待资源
struct StateBlocked : public etl::fsm_state<etl::fsm, StateBlocked,
                                            TaskStatusId::kBlocked, MsgWakeup> {
  auto on_event(const MsgWakeup&) -> etl::fsm_state_id_t {
    return TaskStatusId::kReady;
  }
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t {
    klog::Warn("TaskFsm: Blocked received unexpected message id=%d\n",
               static_cast<int>(msg.get_message_id()));
    return STATE_ID;
  }
};

/// 状态：Exited — 任务已退出，无父任务回收
struct StateExited : public etl::fsm_state<etl::fsm, StateExited,
                                           TaskStatusId::kExited, MsgReap> {
  auto on_event(const MsgReap&) -> etl::fsm_state_id_t { return STATE_ID; }
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t {
    klog::Warn("TaskFsm: Exited received unexpected message id=%d\n",
               static_cast<int>(msg.get_message_id()));
    return STATE_ID;
  }
};

/// 状态：Zombie — 任务已退出，等待父任务回收
struct StateZombie : public etl::fsm_state<etl::fsm, StateZombie,
                                           TaskStatusId::kZombie, MsgReap> {
  auto on_event(const MsgReap&) -> etl::fsm_state_id_t {
    return TaskStatusId::kExited;
  }
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t {
    klog::Warn("TaskFsm: Zombie received unexpected message id=%d\n",
               static_cast<int>(msg.get_message_id()));
    return STATE_ID;
  }
};

class TaskFsm {
 public:
  TaskFsm() : fsm_(router_id::kTaskFsm) {
    state_list_[0] = &state_uninit_;
    state_list_[1] = &state_ready_;
    state_list_[2] = &state_running_;
    state_list_[3] = &state_sleeping_;
    state_list_[4] = &state_blocked_;
    state_list_[5] = &state_exited_;
    state_list_[6] = &state_zombie_;
    fsm_.set_states(state_list_, 7);
  }

  /// 启动 FSM（在 TCB 完全构造后调用）
  void Start() { fsm_.start(); }

  /// 向 FSM 发送消息
  void Receive(const etl::imessage& msg) { fsm_.receive(msg); }

  /// 获取当前状态 ID
  auto GetStateId() const -> etl::fsm_state_id_t { return fsm_.get_state_id(); }

 private:
  StateUnInit state_uninit_;
  StateReady state_ready_;
  StateRunning state_running_;
  StateSleeping state_sleeping_;
  StateBlocked state_blocked_;
  StateExited state_exited_;
  StateZombie state_zombie_;

  etl::ifsm_state* state_list_[7];

  etl::fsm fsm_;
};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_
