/*
 * AArch64 异常向量表 + Trap 入口/返回汇编
 *
 * 布局与 context.rs 中 TrapContext 严格对应，修改时需同步更新。
 */

/*
 * 常量与宏定义
 */

.equ kRegsBytes, 8

/* TrapContext: 320 字节 */
.equ kTrapContextSize, 320

/*
 * 保存 TrapContext：分配栈空间，保存 x0-x30 + 系统寄存器
 *
 * 偏移 0-247: x0-x30
 * 偏移 248: _padding0
 * 偏移 256-271: elr_el1, spsr_el1
 * 偏移 272: esr_el1
 * 偏移 280-295: sp_el0, tpidr_el0
 * 偏移 296: ttbr0_el1
 * 偏移 304-319: sp_el1, tpidr_el1
 */
.macro SaveTrapContext
    sub sp, sp, #kTrapContextSize

    // 保存通用寄存器 x0-x30 (偏移 0-247)
    stp x0, x1, [sp, #16 * 0]
    stp x2, x3, [sp, #16 * 1]
    stp x4, x5, [sp, #16 * 2]
    stp x6, x7, [sp, #16 * 3]
    stp x8, x9, [sp, #16 * 4]
    stp x10, x11, [sp, #16 * 5]
    stp x12, x13, [sp, #16 * 6]
    stp x14, x15, [sp, #16 * 7]
    stp x16, x17, [sp, #16 * 8]
    stp x18, x19, [sp, #16 * 9]
    stp x20, x21, [sp, #16 * 10]
    stp x22, x23, [sp, #16 * 11]
    stp x24, x25, [sp, #16 * 12]
    stp x26, x27, [sp, #16 * 13]
    stp x28, x29, [sp, #16 * 14]
    str x30, [sp, #240]

    // 保存系统寄存器 (偏移 256-319)
    mrs x9, elr_el1
    mrs x10, spsr_el1
    stp x9, x10, [sp, #256]
    mrs x9, esr_el1
    str x9, [sp, #272]

    mrs x9, sp_el0
    mrs x10, tpidr_el0
    stp x9, x10, [sp, #280]
    mrs x9, ttbr0_el1
    str x9, [sp, #296]

    // sp_el1: 保存分配前的栈指针
    add x9, sp, #kTrapContextSize
    mrs x10, tpidr_el1
    stp x9, x10, [sp, #304]
.endm

/*
 * 恢复 TrapContext：从栈上恢复 x0-x30 + 系统寄存器，释放栈空间
 */
.macro RestoreTrapContext
    // 恢复系统寄存器 (偏移 256-319)
    ldp x9, x10, [sp, #256]
    msr elr_el1, x9
    msr spsr_el1, x10
    // 注意：不恢复 esr_el1，它是硬件在异常时自动设置的只读寄存器

    ldp x9, x10, [sp, #280]
    ldr x11, [sp, #296]
    msr sp_el0, x9
    msr tpidr_el0, x10
    msr ttbr0_el1, x11

    ldp x9, x10, [sp, #304]
    msr tpidr_el1, x10

    // 恢复通用寄存器 x0-x30 (偏移 0-247)
    ldp x0, x1, [sp, #16 * 0]
    ldp x2, x3, [sp, #16 * 1]
    ldp x4, x5, [sp, #16 * 2]
    ldp x6, x7, [sp, #16 * 3]
    ldp x8, x9, [sp, #16 * 4]
    ldp x10, x11, [sp, #16 * 5]
    ldp x12, x13, [sp, #16 * 6]
    ldp x14, x15, [sp, #16 * 7]
    ldp x16, x17, [sp, #16 * 8]
    ldp x18, x19, [sp, #16 * 9]
    ldp x20, x21, [sp, #16 * 10]
    ldp x22, x23, [sp, #16 * 11]
    ldp x24, x25, [sp, #16 * 12]
    ldp x26, x27, [sp, #16 * 13]
    ldp x28, x29, [sp, #16 * 14]
    ldr x30, [sp, #240]

    // 恢复 sp (释放栈空间)
    add sp, sp, #kTrapContextSize
.endm

/*
 * 异常向量表 + 处理入口
 *
 * AArch64 向量表要求 0x800 对齐，每个条目 0x80 字节。
 * [Arm ARM §D1.10.2]
 */

.section .text

.global vector_table
.balign 0x800
vector_table:
// Current EL with SP0
.balign 0x80
    b sync_current_el_sp0
.balign 0x80
    b irq_current_el_sp0
.balign 0x80
    b fiq_current_el_sp0
.balign 0x80
    b error_current_el_sp0

// Current EL with SPx
.balign 0x80
    b sync_current_el_spx
.balign 0x80
    b irq_current_el_spx
.balign 0x80
    b fiq_current_el_spx
.balign 0x80
    b error_current_el_spx

// Lower EL using AArch64
.balign 0x80
    b sync_lower_el_aarch64
.balign 0x80
    b irq_lower_el_aarch64
.balign 0x80
    b fiq_lower_el_aarch64
.balign 0x80
    b error_lower_el_aarch64

// Lower EL using AArch32
.balign 0x80
    b sync_lower_el_aarch32
.balign 0x80
    b irq_lower_el_aarch32
.balign 0x80
    b fiq_lower_el_aarch32
.balign 0x80
    b error_lower_el_aarch32

// Exception handlers
sync_current_el_sp0:
    SaveTrapContext
    mov x0, sp
    bl sync_current_el_sp0_handler
    RestoreTrapContext
    eret

irq_current_el_sp0:
    SaveTrapContext
    mov x0, sp
    bl irq_current_el_sp0_handler
    RestoreTrapContext
    eret

fiq_current_el_sp0:
    SaveTrapContext
    mov x0, sp
    bl fiq_current_el_sp0_handler
    RestoreTrapContext
    eret

error_current_el_sp0:
    SaveTrapContext
    mov x0, sp
    bl error_current_el_sp0_handler
    RestoreTrapContext
    eret

sync_current_el_spx:
    SaveTrapContext
    mov x0, sp
    bl sync_current_el_spx_handler
    RestoreTrapContext
    eret

irq_current_el_spx:
    SaveTrapContext
    mov x0, sp
    bl irq_current_el_spx_handler
    RestoreTrapContext
    eret

fiq_current_el_spx:
    SaveTrapContext
    mov x0, sp
    bl fiq_current_el_spx_handler
    RestoreTrapContext
    eret

error_current_el_spx:
    SaveTrapContext
    mov x0, sp
    bl error_current_el_spx_handler
    RestoreTrapContext
    eret

sync_lower_el_aarch64:
    SaveTrapContext
    mov x0, sp
    bl sync_lower_el_aarch64_handler
    RestoreTrapContext
    eret

irq_lower_el_aarch64:
    SaveTrapContext
    mov x0, sp
    bl irq_lower_el_aarch64_handler
    RestoreTrapContext
    eret

fiq_lower_el_aarch64:
    SaveTrapContext
    mov x0, sp
    bl fiq_lower_el_aarch64_handler
    RestoreTrapContext
    eret

error_lower_el_aarch64:
    SaveTrapContext
    mov x0, sp
    bl error_lower_el_aarch64_handler
    RestoreTrapContext
    eret

sync_lower_el_aarch32:
    SaveTrapContext
    mov x0, sp
    bl sync_lower_el_aarch32_handler
    RestoreTrapContext
    eret

irq_lower_el_aarch32:
    SaveTrapContext
    mov x0, sp
    bl irq_lower_el_aarch32_handler
    RestoreTrapContext
    eret

fiq_lower_el_aarch32:
    SaveTrapContext
    mov x0, sp
    bl fiq_lower_el_aarch32_handler
    RestoreTrapContext
    eret

error_lower_el_aarch32:
    SaveTrapContext
    mov x0, sp
    bl error_lower_el_aarch32_handler
    RestoreTrapContext
    eret

// trap_return: 用于从内核返回到用户态或内核态
// 参数：x0 = TrapContext*
.global trap_return
.type trap_return, @function
trap_return:
    // kernel_thread_entry 跳转到这里时，x0 是参数 (TrapContext*)
    // 将 x0 赋值给 sp，切换到 Trap 上下文所在的栈位置
    mov sp, x0

    // 恢复 Trap 上下文
    // 检查 spsr_el1.M[3:0] 判断返回到用户态还是内核态
    // spsr_el1 at offset 264
    ldr x9, [sp, #264]
    // 提取 M[3:0]
    and x9, x9, #0xF
    cbnz x9, .Lret_to_kernel

.Lret_to_user:
    // 返回用户态
    // ttbr0_el1 at offset 296
    ldr x9, [sp, #296]
    msr ttbr0_el1, x9
    isb

    // 恢复通用寄存器和系统寄存器
    RestoreTrapContext
    eret

.Lret_to_kernel:
    // 返回内核态
    RestoreTrapContext
    eret
