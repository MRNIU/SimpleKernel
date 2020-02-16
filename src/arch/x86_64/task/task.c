
// This file is a part of MRNIU/SimpleKernel (https://github.com/MRNIU/SimpleKernel).
//
// task.c for MRNIU/SimpleKernel.

#ifdef __cplusplus
extern "C" {
#endif

#include "string.h"
#include "assert.h"
#include "cpu.hpp"
#include "gdt/include/gdt.h"
#include "mem/pmm.h"
#include "heap/heap.h"
#include "task/task.h"

// 当前任务数量
static uint32_t curr_task_count = 0;
// 全局 pid 值
static pid_t curr_pid = 0;
// 全部任务链表
ListEntry * task_list = NULL;
// 可调度进程链表
ListEntry * runnable_list = NULL;
// 等待进程链表
ListEntry * wait_list = NULL;

// 返回一个空的任务控制块
static task_pcb_t * alloc_task_pcb(void) {
	// 申请内存
	task_pcb_t * task_pcb = (task_pcb_t *)kmalloc(TASK_STACK_SIZE);
	if(task_pcb == NULL) {
		printk_err("Error at task.c: alloc_task_pcb. No enough memory!\n");
		return NULL;
	}
	bzero(task_pcb, sizeof(task_pcb_t) );
	// 填充
	task_pcb->status = TASK_UNINIT;
	task_pcb->pid = ++curr_pid;
	char * name = (char *)kmalloc(TASK_NAME_MAX + 1);
	bzero(name, TASK_NAME_MAX + 1);
	task_pcb->name = name;
	task_pcb->run_time = 0;
	task_pcb->parent = NULL;
	task_mem_t * mm = (task_mem_t *)kmalloc(sizeof(task_mem_t) );
	bzero(mm, sizeof(task_mem_t) );
	task_pcb->mm = mm;
	task_pcb->mm->stack_top = (ptr_t)task_pcb;
	task_pcb->mm->stack_bottom = (ptr_t)task_pcb + TASK_STACK_SIZE;
	// 将 pt_regs 结构体放在任务栈的底部
	task_pcb->pt_regs = (pt_regs_t *)( (ptr_t)task_pcb + TASK_STACK_SIZE - sizeof(pt_regs_t) );
	bzero(task_pcb->pt_regs, sizeof(pt_regs_t) );
	task_context_t * context = (task_context_t *)kmalloc(sizeof(task_context_t) );
	bzero(context, sizeof(task_context_t) );
	task_pcb->context = context;
	task_pcb->exit_code = 0;
	list_append(&task_list, task_pcb);
	curr_task_count++;
	return task_pcb;
}

void task_init(void) {
	cpu_cli();
	// 为内核进程创建信息结构体，位置在内核栈的栈底
	task_pcb_t * kernel_task = (task_pcb_t *)(kernel_stack_top);
	bzero(kernel_task, sizeof(task_pcb_t) );
	kernel_task->status = TASK_RUNNING;
	kernel_task->pid = 1;
	char * name = (char *)kmalloc(TASK_NAME_MAX + 1);
	bzero(name, TASK_NAME_MAX + 1);
	kernel_task->name = name;
	strcpy(kernel_task->name, "Kernel task");
	kernel_task->run_time = 0;
	kernel_task->parent = NULL;
	task_mem_t * mm = (task_mem_t *)kmalloc(sizeof(task_mem_t) );
	bzero(mm, sizeof(task_mem_t) );
	mm->pgd_dir = pgd_kernel;
	mm->stack_top = (ptr_t)kernel_stack_top;
	// 留出 pt_regs 的空间
	mm->stack_bottom = (ptr_t)kernel_stack_bottom - sizeof(pt_regs_t);
	mm->task_start = (ptr_t)&kernel_start;
	mm->code_start = (ptr_t)&kernel_text_start;
	mm->code_end = (ptr_t)&kernel_text_end;
	mm->data_start = (ptr_t)&kernel_data_start;
	mm->data_end = (ptr_t)&kernel_data_end;
	mm->task_end = (ptr_t)&kernel_end;
	kernel_task->mm = mm;
	task_context_t * context = (task_context_t *)kmalloc(sizeof(task_context_t) );
	bzero(context, sizeof(task_context_t) );
	kernel_task->context = context;
	kernel_task->context->esp = (ptr_t)kernel_task + TASK_STACK_SIZE;
	kernel_task->exit_code = 0;
	curr_pid = 1;
	curr_task_count = 1;
	list_append(&task_list, kernel_task);
	list_append(&runnable_list, kernel_task);
	printk_info("task_init\n");
	return;
}

pid_t kernel_thread(int32_t (* fun)(void *), void * args, uint32_t flags) {
	pt_regs_t pt_regs;
	bzero(&pt_regs, sizeof(pt_regs_t) );
	pt_regs.ds = KERNEL_DS;
	pt_regs.es = KERNEL_DS;
	pt_regs.cs = KERNEL_CS;
	pt_regs.user_ss = KERNEL_DS;
	pt_regs.eflags |= EFLAGS_IF;

	pt_regs.edx = (ptr_t)args;
	pt_regs.ebx = (ptr_t)fun;
	pt_regs.eip = (ptr_t)kthread_entry;

	return do_fork(&pt_regs, flags);
}

static void copy_thread(task_pcb_t * task, pt_regs_t * pt_regs) {
	task->pt_regs = (pt_regs_t *)( (ptr_t)task->mm->stack_bottom - sizeof(pt_regs_t) );
	*(task->pt_regs) = *pt_regs;
	task->pt_regs->eax = 0;
	task->pt_regs->user_esp = (uint32_t)task->mm->stack_bottom;
	task->pt_regs->eflags |= EFLAGS_IF;

	task->context->eip = (uint32_t)forkret_s;
	task->context->esp = (uint32_t)task->pt_regs;
}

pid_t do_fork(pt_regs_t * pt_regs, uint32_t flags) {
	if(curr_task_count >= TASK_MAX) {
		return -1;
	}
	task_pcb_t * task = alloc_task_pcb();
	if(task == NULL) {
		return -1;
	}
	copy_thread(task, pt_regs);
	task->pid = ++curr_pid;
	task->status = TASK_RUNNABLE;
	list_append(&runnable_list, task);
	printk_debug("task->pt_regs->edx: 0x%08X\n", task->pt_regs->edx);
	printk_debug("task->pt_regs->ebx: 0x%08X\n", task->pt_regs->ebx);
	printk_debug("task->pt_regs->eip: 0x%08X\n", task->pt_regs->eip);
	printk_debug("task->esp: 0x%08X\n", task->context->esp);
	printk_debug("task->eip: 0x%08X\n", task->context->eip);

	return task->pid;
}

void do_exit(int32_t exit_code) {
	get_current_task()->status = TASK_ZOMBIE;
	get_current_task()->exit_code = exit_code;
	curr_pid--;
	curr_task_count--;
	return;
}

task_pcb_t * get_current_task() {
	register uint32_t esp __asm__ ("esp");
	return (task_pcb_t *)(esp & (~(STACK_SIZE - 1) ) );
}

void kthread_exit() {
	register uint32_t val __asm__ ("eax");
	printk("Thread exited with value %d\n", val);
	while(1);
	return;
}

// 线程创建
pid_t kfork(int (* fn)(void *) UNUSED, void * arg UNUSED) {
	return 0;
}

void kexit() {
	return;
}

#ifdef __cplusplus
}
#endif