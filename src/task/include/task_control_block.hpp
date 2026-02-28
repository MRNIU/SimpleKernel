/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_TASK_CONTROL_BLOCK_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_TASK_CONTROL_BLOCK_HPP_

#include <cpu_io.h>
#include <etl/flags.h>

#include <array>
#include <cstddef>
#include <cstdint>

#include "file_descriptor.hpp"
#include "resource_id.hpp"

/// 进程 ID 类型
using Pid = size_t;

/// 线程入口函数类型
using ThreadEntry = void (*)(void*);

/**
 * @brief Clone 标志位常量 (用于 sys_clone 系统调用)
 * @note 参考 Linux clone flags
 */
namespace clone_flag {
// 共享地址空间
inline constexpr uint64_t kVm = 0x00000100;
// 共享文件系统信息
inline constexpr uint64_t kFs = 0x00000200;
// 共享文件描述符表
inline constexpr uint64_t kFiles = 0x00000400;
// 共享信号处理器
inline constexpr uint64_t kSighand = 0x00000800;
// 保持相同父进程
inline constexpr uint64_t kParent = 0x00008000;
// 同一线程组
inline constexpr uint64_t kThread = 0x00010000;
// 全部标志掩码
inline constexpr uint64_t kAllMask =
    kVm | kFs | kFiles | kSighand | kParent | kThread;
}  // namespace clone_flag

/// @brief 类型安全的克隆标志位
using CloneFlags = etl::flags<uint64_t, clone_flag::kAllMask>;

/// @brief 类型安全的 CPU 亲和性位掩码
using CpuAffinity = etl::flags<uint64_t>;

/**
 * @brief 任务状态枚举
 */
enum TaskStatus : uint8_t {
  // 未初始化
  kUnInit,
  // 就绪
  kReady,
  // 正在运行
  kRunning,
  // 睡眠中
  kSleeping,
  // 阻塞
  kBlocked,
  // 已退出
  kExited,
  // 僵尸状态 (等待回收)
  kZombie,
};

/**
 * @brief 调度策略
 */
enum SchedPolicy : uint8_t {
  // 实时任务 (最高优先级)
  kRealTime = 0,
  // 普通任务
  kNormal = 1,
  // 空闲任务 (最低优先级)
  kIdle = 2,
  // 策略数量
  kPolicyCount
};

/**
 * @brief 任务控制块，管理进程/线程的核心数据结构
 */
struct TaskControlBlock {
  /// 默认内核栈大小 (16 KB)
  static constexpr const size_t kDefaultKernelStackSize = 16 * 1024;

  /// 任务优先级比较函数，优先级数值越小，优先级越高
  struct PriorityCompare {
    bool operator()(TaskControlBlock* a, TaskControlBlock* b) {
      return a->sched_info.priority > b->sched_info.priority;
    }
  };

  /// 任务唤醒时间比较函数，时间越早优先级越高
  struct WakeTickCompare {
    bool operator()(TaskControlBlock* a, TaskControlBlock* b) {
      return a->sched_info.wake_tick > b->sched_info.wake_tick;
    }
  };

  /// 任务名称
  const char* name = "Unnamed Task";

  /// 线程 ID (Task ID)
  Pid pid = 0;
  /// 父线程 ID
  Pid parent_pid = 0;
  /// 进程组 ID
  Pid pgid = 0;
  /// 会话 ID
  Pid sid = 0;
  /// 线程组 ID (主线程的 PID)
  Pid tgid = 0;

  /// @name 侵入式线程组链表
  /// @{
  TaskControlBlock* thread_group_next = nullptr;
  TaskControlBlock* thread_group_prev = nullptr;
  /// @}

  /// 线程状态
  TaskStatus status = TaskStatus::kUnInit;
  /// 调度策略
  SchedPolicy policy = SchedPolicy::kNormal;

  /// 退出码
  int exit_code = 0;

  /// 克隆标志位
  CloneFlags clone_flags{};

  /**
   * @brief 基础调度信息
   */
  struct SchedInfo {
    /// 优先级 (数字越小优先级越高)
    int priority = 10;
    /// 基础优先级 (静态，用于优先级继承)
    int base_priority = 10;
    /// 继承的优先级
    int inherited_priority = 0;
    /// 唤醒时间
    uint64_t wake_tick = 0;
    /// 剩余时间片
    uint64_t time_slice_remaining = 10;
    /// 默认时间片
    uint64_t time_slice_default = 10;
    /// 总运行时间
    uint64_t total_runtime = 0;
    /// 上下文切换次数
    uint64_t context_switches = 0;
  } sched_info;

  /**
   * @brief 不同调度器的专用字段 (互斥使用)
   */
  union SchedData {
    /// CFS 调度器数据
    struct {
      /// 虚拟运行时间
      uint64_t vruntime;
      /// 任务权重 (1024 为默认)
      uint32_t weight;
    } cfs;

    /// MLFQ 调度器数据
    struct {
      /// 优先级级别 (0 = 最高)
      uint8_t level;
    } mlfq;
  } sched_data{};

  /// 内核栈
  uint8_t* kernel_stack = nullptr;

  /// Trap 上下文
  cpu_io::TrapContext* trap_context_ptr = nullptr;
  /// 任务上下文
  cpu_io::CalleeSavedContext task_context{};
  /// 页表
  uint64_t* page_table = nullptr;

  /// CPU 亲和性位掩码
  CpuAffinity cpu_affinity{UINT64_MAX};

  /// 等待的资源 ID
  ResourceId blocked_on{};

  /// 是否为中断线程
  bool is_interrupt_thread = false;
  /// 关联的中断号
  uint64_t interrupt_number = 0;

  /// @todo 优先级继承相关

  /// 文件描述符表
  filesystem::FileDescriptorTable* fd_table = nullptr;

  /// @name 线程组相关方法
  /// @{
  /**
   * @brief 检查是否是线程组的主线程
   * @return true 如果是主线程 (pid == tgid)
   */
  auto IsThreadGroupLeader() const -> bool { return pid == tgid; }

  /**
   * @brief 将线程添加到线程组
   * @param leader 线程组的主线程
   * @note 调用者需要确保加锁
   */
  void JoinThreadGroup(TaskControlBlock* leader);

  /**
   * @brief 从线程组中移除自己
   * @note 调用者需要确保加锁
   */
  void LeaveThreadGroup();

  /**
   * @brief 获取线程组中的线程数量
   * @return size_t 线程数量 (包括自己)
   */
  auto GetThreadGroupSize() const -> size_t;

  /**
   * @brief 检查是否与另一个任务在同一线程组
   * @param other 另一个任务
   * @return true 如果在同一线程组
   */
  auto InSameThreadGroup(const TaskControlBlock* other) const -> bool {
    return other && (tgid == other->tgid) && (tgid != 0);
  }
  /// @}

  /**
   * @brief 构造函数 (内核线程)
   * @param name 任务名称
   * @param priority 优先级 (数字越小优先级越高)
   * @param entry 线程入口函数
   * @param arg 线程参数
   */
  TaskControlBlock(const char* name, int priority, ThreadEntry entry,
                   void* arg);

  /**
   * @brief 构造函数 (用户线程)
   * @param name 任务名称
   * @param priority 优先级 (数字越小优先级越高)
   * @param elf 指向 ELF 镜像的指针
   * @param argc 参数个数
   * @param argv 参数数组
   */
  TaskControlBlock(const char* name, int priority, uint8_t* elf, int argc,
                   char** argv);

  /// @name 构造/析构函数
  /// @{
  TaskControlBlock() = default;
  TaskControlBlock(const TaskControlBlock&) = delete;
  TaskControlBlock(TaskControlBlock&&) = delete;
  auto operator=(const TaskControlBlock&) -> TaskControlBlock& = delete;
  auto operator=(TaskControlBlock&&) -> TaskControlBlock& = delete;
  ~TaskControlBlock();
  /// @}
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_TASK_CONTROL_BLOCK_HPP_ */
