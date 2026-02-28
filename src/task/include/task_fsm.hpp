/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_

#include <etl/fsm.h>

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
class StateUnInit;
class StateReady;
class StateRunning;
class StateSleeping;
class StateBlocked;
class StateExited;
class StateZombie;

/// 状态：UnInit — 任务尚未初始化
class StateUnInit : public etl::fsm_state<etl::fsm, StateUnInit,
                                          TaskStatusId::kUnInit, MsgSchedule> {
 public:
  auto on_event(const MsgSchedule& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

/// 状态：Ready — 任务已就绪，等待调度
class StateReady : public etl::fsm_state<etl::fsm, StateReady,
                                         TaskStatusId::kReady, MsgSchedule> {
 public:
  auto on_event(const MsgSchedule& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

/// 状态：Running — 任务正在执行
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

/// 状态：Sleeping — 任务已挂起，等待唤醒时钟
class StateSleeping
    : public etl::fsm_state<etl::fsm, StateSleeping, TaskStatusId::kSleeping,
                            MsgWakeup> {
 public:
  auto on_event(const MsgWakeup& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

/// 状态：Blocked — 任务阻塞，等待资源
class StateBlocked : public etl::fsm_state<etl::fsm, StateBlocked,
                                           TaskStatusId::kBlocked, MsgWakeup> {
 public:
  auto on_event(const MsgWakeup& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

/// 状态：Exited — 任务已退出，无父任务回收
class StateExited : public etl::fsm_state<etl::fsm, StateExited,
                                          TaskStatusId::kExited, MsgReap> {
 public:
  auto on_event(const MsgReap& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

/// 状态：Zombie — 任务已退出，等待父任务回收
class StateZombie : public etl::fsm_state<etl::fsm, StateZombie,
                                          TaskStatusId::kZombie, MsgReap> {
 public:
  auto on_event(const MsgReap& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

class TaskFsm {
 public:
  TaskFsm();

  /// 启动 FSM（在 TCB 完全构造后调用）
  void Start();

  /// 向 FSM 发送消息
  void Receive(const etl::imessage& msg);

  /// 获取当前状态 ID
  auto GetStateId() const -> etl::fsm_state_id_t;

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

#endif /* SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_ */
