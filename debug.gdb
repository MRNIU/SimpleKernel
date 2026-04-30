set confirm off
set pagination off
set print pretty on
set breakpoint pending on

target remote :1234

rbreak simplekernel::task::sched::schedule
rbreak switch_to
rbreak simplekernel::timer::handle_timer_common

define sk-bt
    info registers
    bt
end

document sk-bt
打印当前寄存器和调用栈。
end

define sk-current-task
    printf "该命令会在目标机上调用 Rust 函数；仅在 TaskInit 完成后使用。\n"
    p per_cpu::current_core_id()
    p simplekernel::task::current_task()
end

document sk-current-task
打印当前 CPU 和当前任务。需要目标机已完成任务系统初始化。
end
