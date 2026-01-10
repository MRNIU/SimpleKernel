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
  // 初始化调度器数组
  schedulers[SchedPolicy::kRealTime] = new RtScheduler();
  schedulers[SchedPolicy::kNormal] = new FifoScheduler();
  schedulers[SchedPolicy::kIdle] = nullptr;
}

void TaskManager::AddTask(TaskControlBlock* task) {
  if (task->policy < SchedPolicy::kPolicyCount) {
    schedulers[task->policy]->Enqueue(task);
  }
}

void TaskManager::Schedule() {
  TaskControlBlock* next_task = nullptr;

  // 1. 如果当前任务还在运行 (Yield的情况)，先将其放回就绪队列
  if (current_task->status == TaskStatus::kRunning) {
    current_task->status = TaskStatus::kReady;
    if (current_task->policy < SchedPolicy::kPolicyCount &&
        schedulers[current_task->policy]) {
      schedulers[current_task->policy]->Enqueue(current_task);
    }
  }

  // 2. 按优先级遍历调度器，寻找下一个任务
  // 顺序: RealTime -> Normal -> Idle (如果有)
  for (auto* sched : schedulers) {
    if (sched) {
      next_task = sched->PickNext();
      if (next_task) {
        break;
      }
    }
  }

  if (!next_task) {
    // 如果当前任务被放回去了，那它就是唯一的任务了，继续跑它
    if (current_task->status == TaskStatus::kReady) {
      current_task->status = TaskStatus::kRunning;
      return;
    }

    // 如果没有任务就绪（且当前任务也在睡眠），则等待
    // 这里简单实现为忙等待，直到有任务就绪 (由中断触发 UpdateTick 唤醒)
    while (!next_task) {
      for (auto* sched : schedulers) {
        if (sched) {
          next_task = sched->PickNext();
          if (next_task) break;
        }
      }
    }
  }

  TaskControlBlock* prev_task = current_task;
  current_task = next_task;
  current_task->status = TaskStatus::kRunning;

  // 执行上下文切换
  switch_to(&prev_task->task_context, &current_task->task_context);
}

void TaskManager::InitMainThread() {
  static TaskControlBlock main_task;  // Idle/Main thread
  main_task.pid = 0;
  main_task.status = TaskStatus::kRunning;
  main_task.policy = SchedPolicy::kNormal;  // 初始设为 Normal
  current_task = &main_task;
}

void TaskManager::UpdateTick() {
  current_tick++;
  auto it = sleeping_tasks.begin();
  while (it != sleeping_tasks.end()) {
    TaskControlBlock* task = *it;
    if (current_tick >= task->wake_tick) {
      task->status = TaskStatus::kReady;
      AddTask(task);
      it = sleeping_tasks.erase(it);
    } else {
      ++it;
    }
  }
}

void TaskManager::Sleep(uint64_t ms) {
  if (!current_task) return;

  // 至少睡眠 1 tick
  uint64_t ticks = ms * tick_frequency / 1000;
  if (ticks == 0) ticks = 1;

  current_task->wake_tick = current_tick + ticks;
  current_task->status = TaskStatus::kSleeping;
  sleeping_tasks.push_back(current_task);

  Schedule();
}
