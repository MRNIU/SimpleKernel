/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_ARCH_ARCH_H_
#define SIMPLEKERNEL_SRC_ARCH_ARCH_H_

#include <cpu_io.h>
#include <sys/cdefs.h>

#include <cstddef>
#include <cstdint>

// 在 switch.S 中定义
extern "C" void switch_to(cpu_io::CalleeSavedContext* prev,
                          cpu_io::CalleeSavedContext* next);

// 在 switch.S 中定义
extern "C" void kernel_thread_entry();

// 在 switch.S 中定义
extern "C" void trap_return(void*);

// 在 interrupt.S 中定义
extern "C" void trap_entry();

/**
 * @brief 体系结构相关初始化
 * @param argc 在不同体系结构有不同含义，同 _start
 * @param argv 在不同体系结构有不同含义，同 _start
 * @pre 引导程序已完成基本硬件初始化
 * @post 架构相关硬件（串口、内存、设备树等）已初始化
 */
void ArchInit(int argc, const char** argv);
/**
 * @brief 从核的体系结构相关初始化
 * @param argc 在不同体系结构有不同含义，同 _start
 * @param argv 在不同体系结构有不同含义，同 _start
 * @pre 主核已完成 ArchInit
 * @post 从核的架构相关硬件已初始化
 */
void ArchInitSMP(int argc, const char** argv);

/**
 * @brief 唤醒其余 core
 * @pre 主核已完成初始化
 * @post 所有从核开始执行 ArchInitSMP
 */
void WakeUpOtherCores();

/**
 * @brief 体系结构相关中断初始化
 * @param argc 在不同体系结构有不同含义，同 _start
 * @param argv 在不同体系结构有不同含义，同 _start
 * @pre ArchInit 已完成
 * @post 中断控制器已初始化，中断向量表已设置
 */
void InterruptInit(int argc, const char** argv);
/**
 * @brief 从核的体系结构相关中断初始化
 * @param argc 在不同体系结构有不同含义，同 _start
 * @param argv 在不同体系结构有不同含义，同 _start
 * @pre 主核已完成 InterruptInit
 * @post 从核的中断控制器已初始化
 */
void InterruptInitSMP(int argc, const char** argv);
/**
 * @brief 初始化定时器
 * @pre InterruptInit 已完成
 * @post 定时器中断已启用，系统 tick 开始计数
 */
void TimerInit();
/**
 * @brief 从核的定时器初始化
 * @pre 主核已完成 TimerInit，从核已完成 InterruptInitSMP
 * @post 从核的定时器中断已启用
 */
void TimerInitSMP();

/**
 * @brief 初始化内核线程的任务上下文（重载1）
 * @param task_context 指向任务上下文的指针
 * @param entry 线程入口函数
 * @param arg 传递给线程的参数
 * @param stack_top 内核栈顶地址
 * @pre task_context 不为 nullptr，stack_top 已按架构要求对齐
 * @post task_context 已设置为可被 switch_to 恢复的状态
 */
void InitTaskContext(cpu_io::CalleeSavedContext* task_context,
                     void (*entry)(void*), void* arg, uint64_t stack_top);

/**
 * @brief 初始化用户线程的任务上下文（重载2）
 * @param task_context 指向任务上下文的指针
 * @param trap_context_ptr 指向 Trap 上下文的指针
 * @param stack_top 内核栈顶地址
 * @pre task_context 不为 nullptr，trap_context_ptr 已填充用户态寄存器
 * @post task_context 已设置为经由 trap_return 返回用户态的状态
 */
void InitTaskContext(cpu_io::CalleeSavedContext* task_context,
                     cpu_io::TrapContext* trap_context_ptr, uint64_t stack_top);

/// 最多回溯 128 层调用栈
static constexpr size_t kMaxFrameCount = 128;

/**
 * @brief 获取调用栈
 * @param buffer 指向一个数组，该数组用于存储调用栈中的返回地址
 * @param size 数组的大小，即调用栈中最多存储多少个返回地址
 * @return 成功时返回实际写入数组中的地址数量，失败时返回 -1
 * @pre buffer 不为 nullptr，size > 0
 * @post buffer 中包含从当前帧开始的返回地址序列
 */
__always_inline auto backtrace(void** buffer, int size) -> int;

/**
 * @brief 打印调用栈
 * @note 调用 backtrace 获取调用栈并解析符号后输出到日志
 */
void DumpStack();

#endif /* SIMPLEKERNEL_SRC_ARCH_ARCH_H_ */
