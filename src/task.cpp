/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task.hpp"

#include <cpu_io.h>

#include <algorithm>
#include <memory>

#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "scheduler/fifo_scheduler.hpp"
#include "singleton.hpp"
#include "sk_cstring"
#include "sk_new"
#include "sk_stdlib.h"
#include "sk_vector"
#include "virtual_memory.hpp"

static uint64_t LoadElf(const uint8_t* elf_data, uint64_t* page_table) {
  // Check ELF magic
  auto* ehdr = reinterpret_cast<const Elf64_Ehdr*>(elf_data);
  if (ehdr->e_ident[EI_MAG0] != ELFMAG0 || ehdr->e_ident[EI_MAG1] != ELFMAG1 ||
      ehdr->e_ident[EI_MAG2] != ELFMAG2 || ehdr->e_ident[EI_MAG3] != ELFMAG3) {
    klog::Err("Invalid ELF magic\n");
    return 0;
  }

  auto* phdr = reinterpret_cast<const Elf64_Phdr*>(elf_data + ehdr->e_phoff);
  auto& vm = Singleton<VirtualMemory>::GetInstance();

  for (int i = 0; i < ehdr->e_phnum; ++i) {
    if (phdr[i].p_type != PT_LOAD) continue;

    uintptr_t vaddr = phdr[i].p_vaddr;
    uintptr_t memsz = phdr[i].p_memsz;
    uintptr_t filesz = phdr[i].p_filesz;
    uintptr_t offset = phdr[i].p_offset;

    uint32_t flags = cpu_io::virtual_memory::GetUserPagePermissions(
        (phdr[i].p_flags & PF_R) != 0, (phdr[i].p_flags & PF_W) != 0,
        (phdr[i].p_flags & PF_X) != 0);

    uintptr_t start_page = cpu_io::virtual_memory::PageAlign(vaddr);
    uintptr_t end_page = cpu_io::virtual_memory::PageAlignUp(vaddr + memsz);

    for (uintptr_t page = start_page; page < end_page;
         page += cpu_io::virtual_memory::kPageSize) {
      void* p_page = aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                   cpu_io::virtual_memory::kPageSize);
      if (!p_page) {
        klog::Err("Failed to allocate page for ELF\n");
        return 0;
      }
      std::memset(p_page, 0, cpu_io::virtual_memory::kPageSize);

      // Mapping logic
      uintptr_t v_start = page;
      uintptr_t v_end = page + cpu_io::virtual_memory::kPageSize;

      // Map intersection with file data
      uintptr_t file_start = vaddr;
      uintptr_t file_end = vaddr + filesz;

      uintptr_t copy_start = std::max(v_start, file_start);
      uintptr_t copy_end = std::min(v_end, file_end);

      if (copy_end > copy_start) {
        uintptr_t dst_off = copy_start - v_start;
        uintptr_t src_off = (copy_start - vaddr) + offset;
        std::memcpy((uint8_t*)p_page + dst_off, elf_data + src_off,
                    copy_end - copy_start);
      }

      if (!vm.MapPage(page_table, (void*)page, p_page, flags)) {
        klog::Err("MapPage failed\n");
        return 0;
      }
    }
  }
  return ehdr->e_entry;
};

TaskControlBlock::TaskControlBlock(const char* name, size_t pid,
                                   ThreadEntry entry, void* arg)
    : name(name),
      pid(pid),
      trap_context_ptr(reinterpret_cast<cpu_io::TrapContext*>(
          kernel_stack_top.data() + kernel_stack_top.size() -
          sizeof(cpu_io::TrapContext))) {
  // 设置内核栈顶
  auto stack_top = reinterpret_cast<uint64_t>(kernel_stack_top.data()) +
                   kernel_stack_top.size();

  // 1. 设置 ra 指向汇编跳板 kernel_thread_entry
  // 当 switch_to 执行 ret 时，会跳转到 kernel_thread_entry
  task_context.ra = reinterpret_cast<uint64_t>(kernel_thread_entry);

  // 2. 设置 s0 保存真正的入口函数地址
  task_context.s0 = reinterpret_cast<uint64_t>(entry);

  // 3. 设置 s1 保存参数
  task_context.s1 = reinterpret_cast<uint64_t>(arg);

  // 4. 设置 sp 为栈顶
  task_context.sp = stack_top;

  // 状态设为就绪
  status = TaskStatus::kReady;
}

TaskControlBlock::TaskControlBlock(const char* name, size_t pid, uint8_t* elf,
                                   int argc, char** argv)
    : name(name),
      pid(pid),
      trap_context_ptr(reinterpret_cast<cpu_io::TrapContext*>(
          kernel_stack_top.data() + kernel_stack_top.size() -
          sizeof(cpu_io::TrapContext))) {
  // 1. 创建页表
  page_table = reinterpret_cast<uint64_t*>(aligned_alloc(
      cpu_io::virtual_memory::kPageSize, cpu_io::virtual_memory::kPageSize));
  // 复制内核映射
  auto current_pgd = cpu_io::virtual_memory::GetPageDirectory();
  std::memcpy(page_table, reinterpret_cast<void*>(current_pgd),
              cpu_io::virtual_memory::kPageSize);

  // 2. 加载 ELF
  uint64_t entry_point = LoadElf(elf, page_table);
  if (!entry_point) {
    status = TaskStatus::kExited;
    return;
  }

  // 3. 分配用户栈 (这里简化为分配一页作为栈)
  auto& vm = Singleton<VirtualMemory>::GetInstance();
  // 假设用户栈顶
  constexpr uintptr_t kUserStackTop = 0x80000000;
  void* stack_page = aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                   cpu_io::virtual_memory::kPageSize);
  vm.MapPage(page_table,
             (void*)(kUserStackTop - cpu_io::virtual_memory::kPageSize),
             stack_page,
             cpu_io::virtual_memory::GetUserPagePermissions(true, true, false));

  // 4. 处理参数 (放入栈中)
  uint8_t* sp = (uint8_t*)stack_page + cpu_io::virtual_memory::kPageSize;
  sk_std::vector<uint64_t> argv_addrs;
  // 推入字符串
  for (int i = 0; i < argc; ++i) {
    size_t len = strlen(argv[i]) + 1;
    sp -= len;
    strcpy((char*)sp, argv[i]);
    // 记录用户空间地址
    argv_addrs.push_back(kUserStackTop - cpu_io::virtual_memory::kPageSize +
                         (sp - (uint8_t*)stack_page));
  }
  // 对齐
  sp = (uint8_t*)((uint64_t)sp & ~7);
  // 推入 argv 数组
  sp -= sizeof(uint64_t) * (argc + 1);
  uint64_t* argv_ptr = (uint64_t*)sp;
  for (int i = 0; i < argc; ++i) {
    argv_ptr[i] = argv_addrs[i];
  }
  argv_ptr[argc] = 0;  // NULL terminated

  // 计算最终 sp (用户虚拟地址)
  uint64_t user_sp = kUserStackTop - cpu_io::virtual_memory::kPageSize +
                     (sp - (uint8_t*)stack_page);

  // 5. 初始化 Trap 上下文
  std::memset(trap_context_ptr, 0, sizeof(cpu_io::TrapContext));
#ifdef __riscv
  // sstatus: SPIE=1, SPP=0
  trap_context_ptr->sstatus = 1ULL << 5;
  trap_context_ptr->sepc = entry_point;
  trap_context_ptr->sp = user_sp;
  trap_context_ptr->a0 = argc;
  trap_context_ptr->a1 = user_sp;
#endif

  // 6. 初始化内核切换上下文
  auto stack_top = reinterpret_cast<uint64_t>(kernel_stack_top.data()) +
                   kernel_stack_top.size();
  task_context.ra = reinterpret_cast<uint64_t>(kernel_thread_entry);
  task_context.s0 = reinterpret_cast<uint64_t>(trap_return);
  task_context.s1 = reinterpret_cast<uint64_t>(trap_context_ptr);
  task_context.sp = stack_top;

  // 设置 TrapContext 内容
  // 我们需要确认 TrapContext 布局
  // 如果不知道 sstatus 位置，可能无法正确设置。
  // 但 trap_return 会恢复所有寄存器。
  // 对于 RISC-V, sstatus 通常在 saved registers 中。
  // 再次读取 context.hpp 确认。

  status = TaskStatus::kReady;

  // 修正 Sstatus
  // trap_context_ptr->sstatus = 1ULL << 5; // SPIE
  // trap_context_ptr->sepc = entry_point;
  // trap_context_ptr->x[2] = user_sp; // sp
  // trap_context_ptr->a0 = argc;
  // trap_context_ptr->a1 = user_sp (argv ptr); // argv is at user_sp
}

TaskManager::TaskManager() {
  // 初始化每个核心的调度器
  for (size_t i = 0; i < per_cpu::PerCpu::kMaxCoreCount; ++i) {
    auto& cpu_sched = cpu_schedulers_[i];
    cpu_sched.schedulers[SchedPolicy::kRealTime] = new RtScheduler();
    cpu_sched.schedulers[SchedPolicy::kNormal] = new FifoScheduler();
    // Idle 策略可以有一个专门的调度器，或者直接使用 idle_task
    cpu_sched.schedulers[SchedPolicy::kIdle] = nullptr;

    // 关联到 PerCpu
    // 注意：这里假设 PerCpu 数组已经初始化。
    // 由于 PerCpu 是 Singleton，且通常在早期访问，这里应该是安全的。
    // 但是要小心 TaskManager 初始化时机是否晚于 PerCpu 内存分配。
    // 另一种方式是在 GetCurrentCpuSched 中懒加载或者动态查找。
    // 为了简单，这里通过 Singleton 获取 PerCpu 并设置指针。
    auto& per_cpu_data =
        Singleton<std::array<per_cpu::PerCpu,
                             per_cpu::PerCpu::kMaxCoreCount>>::GetInstance()[i];
    per_cpu_data.sched_data = &cpu_sched;
  }
}

void TaskManager::AddTask(TaskControlBlock* task) {
  // 简单的负载均衡：如果指定了亲和性，放入对应核心，否则放入当前核心
  // 更复杂的逻辑可以是：寻找最空闲的核心
  size_t target_core = cpu_io::GetCurrentCoreId();

  if (task->cpu_affinity != UINT64_MAX) {
    // 简化的亲和性处理：寻找第一个允许的核心
    for (size_t i = 0; i < per_cpu::PerCpu::kMaxCoreCount; ++i) {
      if (task->cpu_affinity & (1UL << i)) {
        target_core = i;
        break;
      }
    }
  }

  auto& cpu_sched = cpu_schedulers_[target_core];

  // 加锁保护运行队列
  cpu_sched.lock.lock();

  if (task->policy < SchedPolicy::kPolicyCount) {
    if (cpu_sched.schedulers[task->policy]) {
      cpu_sched.schedulers[task->policy]->Enqueue(task);
    }
  }

  cpu_sched.lock.unlock();
}

void TaskManager::Schedule() {
  auto& cpu_sched = GetCurrentCpuSched();
  auto& cpu_data = per_cpu::GetCurrentCore();

  // 必须关中断并持有锁才能操作运行队列
  cpu_sched.lock.lock();

  TaskControlBlock* current_task = cpu_data.running_task;
  TaskControlBlock* next_task = nullptr;

  // 1. 如果当前任务还在运行 (Yield的情况)，先将其放回就绪队列
  if (current_task && current_task->status == TaskStatus::kRunning) {
    current_task->status = TaskStatus::kReady;
    // 不要在此时将 idle task 放回队列，因为它特殊处理
    if (current_task != cpu_data.idle_task) {
      if (current_task->policy < SchedPolicy::kPolicyCount &&
          cpu_sched.schedulers[current_task->policy]) {
        cpu_sched.schedulers[current_task->policy]->Enqueue(current_task);
      }
    }
  }

  // 2. 按优先级遍历调度器，寻找下一个任务
  for (auto* sched : cpu_sched.schedulers) {
    if (sched) {
      next_task = sched->PickNext();
      if (next_task) {
        break;
      }
    }
  }

  if (!next_task) {
    // 如果没有普通任务，尝试从其他核心窃取 (Work Stealing)
    // 暂时需解锁以避免死锁 (如果 Balance 内部需要获取其他锁)
    cpu_sched.lock.unlock();
    Balance();
    cpu_sched.lock.lock();

    // 再次尝试获取
    for (auto* sched : cpu_sched.schedulers) {
      if (sched) {
        next_task = sched->PickNext();
        if (next_task) {
          break;
        }
      }
    }
  }

  if (!next_task) {
    // 仍然没有任务，运行 Idle 任务
    // 如果当前就是 Idle 且 Ready/Running，继续运行
    next_task = cpu_data.idle_task;

    // 如果连 idle task 都没有 (启动阶段?)，则需要 panic 或者等待
    if (!next_task) {
      // Should not happen after InitMainThread
      cpu_sched.lock.unlock();
      return;
    }
  }

  TaskControlBlock* prev_task = current_task;
  cpu_data.running_task = next_task;
  next_task->status = TaskStatus::kRunning;

  cpu_sched.lock.unlock();

  // 如果任务变了，切换
  if (prev_task != next_task) {
    switch_to(&prev_task->task_context, &next_task->task_context);
  }
}

void TaskManager::InitCurrentCore() {
  auto* main_task = new TaskControlBlock();
  main_task->name = "Idle/Main";

  // 所有核心的 Idle 任务都使用 PID 0
  main_task->pid = 0;

  main_task->status = TaskStatus::kRunning;
  main_task->policy = SchedPolicy::kIdle;
  main_task->cpu_affinity = (1UL << cpu_io::GetCurrentCoreId());

  auto& cpu_data = per_cpu::GetCurrentCore();
  cpu_data.running_task = main_task;
  cpu_data.idle_task = main_task;
}

size_t TaskManager::AllocatePid() { return pid_allocator.fetch_add(1); }

void TaskManager::UpdateTick() {
  // 原子增加全局 tick? 或者只作为计时。
  // 注意：如果是 Per-Cpu 计时，这里应该只更新 cpu_sched.sleeping_tasks
  current_tick++;

  auto& cpu_sched = GetCurrentCpuSched();
  cpu_sched.lock.lock();

  auto it = cpu_sched.sleeping_tasks.begin();
  while (it != cpu_sched.sleeping_tasks.end()) {
    TaskControlBlock* task = *it;
    if (current_tick >= task->wake_tick) {
      task->status = TaskStatus::kReady;
      // 唤醒到本核心队列 (或者根据 Affinity)
      // 简单起见，因为是在本核心 sleep 的，wake 到本核心
      if (task->policy < SchedPolicy::kPolicyCount &&
          cpu_sched.schedulers[task->policy]) {
        cpu_sched.schedulers[task->policy]->Enqueue(task);
      }

      it = cpu_sched.sleeping_tasks.erase(it);
    } else {
      ++it;
    }
  }

  cpu_sched.lock.unlock();
}

void TaskManager::Sleep(uint64_t ms) {
  auto& cpu_sched = GetCurrentCpuSched();
  auto& cpu_data = per_cpu::GetCurrentCore();
  TaskControlBlock* current = cpu_data.running_task;

  if (!current || current == cpu_data.idle_task) return;

  // 至少睡眠 1 tick
  uint64_t ticks = ms * tick_frequency / 1000;
  if (ticks == 0) ticks = 1;

  // 加锁修改状态
  cpu_sched.lock.lock();
  current->wake_tick = current_tick + ticks;
  current->status = TaskStatus::kSleeping;
  cpu_sched.sleeping_tasks.push_back(current);
  cpu_sched.lock.unlock();

  Schedule();
}

void TaskManager::Balance() {
  // 算法留空
  // TODO: 检查其他核心的运行队列长度，如果比当前核心长，则窃取任务
}
