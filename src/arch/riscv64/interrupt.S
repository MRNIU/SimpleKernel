/*
 * RISC-V 64 Trap 入口/返回汇编
 *
 * 布局与 context.rs 中 TrapContext 严格对应，修改时需同步更新。
 */

/*
 * 常量与宏定义
 */

.equ kRegsBytes, 8

/* 31 个通用寄存器(x1-x31) + 4 个 CSR + 1 个 padding = 36 */
.equ kTrapContextRegsCount, 36

/* 36 * 8 = 288 字节（已 16 字节对齐） */
.equ kTrapContextSize, ((kTrapContextRegsCount * kRegsBytes + 15) & ~15)

.macro SdBase a, b, c
    sd \a, ((\b) * kRegsBytes)(\c)
.endm

.macro LdBase a, b, c
    ld \a, ((\b) * kRegsBytes)(\c)
.endm

/*
 * 保存 TrapContext：分配栈空间，保存 x1-x31 + CSR
 */
.macro SaveTrapContext
    addi sp, sp, -kTrapContextSize

    // 保存通用寄存器 (除了 sp)
    SdBase ra, 0, sp
    // sp 需要特殊处理，这里先跳过，最后计算保存
    SdBase gp, 2, sp
    SdBase tp, 3, sp
    SdBase t0, 4, sp
    SdBase t1, 5, sp
    SdBase t2, 6, sp
    SdBase s0, 7, sp
    SdBase s1, 8, sp
    SdBase a0, 9, sp
    SdBase a1, 10, sp
    SdBase a2, 11, sp
    SdBase a3, 12, sp
    SdBase a4, 13, sp
    SdBase a5, 14, sp
    SdBase a6, 15, sp
    SdBase a7, 16, sp
    SdBase s2, 17, sp
    SdBase s3, 18, sp
    SdBase s4, 19, sp
    SdBase s5, 20, sp
    SdBase s6, 21, sp
    SdBase s7, 22, sp
    SdBase s8, 23, sp
    SdBase s9, 24, sp
    SdBase s10, 25, sp
    SdBase s11, 26, sp
    SdBase t3, 27, sp
    SdBase t4, 28, sp
    SdBase t5, 29, sp
    SdBase t6, 30, sp

    // 保存原来的 sp 到 offset 1
    addi t0, sp, kTrapContextSize
    SdBase t0, 1, sp

    // 保存 CSRs (使用 t0 作为临时寄存器，t0 原值已保存在 offset 4)
    csrr t0, sstatus
    SdBase t0, 31, sp
    csrr t0, sepc
    SdBase t0, 32, sp
    csrr t0, stval
    SdBase t0, 33, sp
    csrr t0, scause
    SdBase t0, 34, sp
.endm

/*
 * 恢复 TrapContext：从栈上恢复 x1-x31 + CSR，释放栈空间
 */
.macro RestoreTrapContext
    // 恢复 CSRs (使用 t0 作为临时寄存器)
    LdBase t0, 31, sp
    csrw sstatus, t0
    LdBase t0, 32, sp
    csrw sepc, t0
    LdBase t0, 33, sp
    csrw stval, t0
    LdBase t0, 34, sp
    csrw scause, t0

    // 恢复通用寄存器 (除了 sp 和 t0)
    LdBase ra, 0, sp
    LdBase gp, 2, sp
    LdBase tp, 3, sp
    // t0 (4) 最后恢复
    LdBase t1, 5, sp
    LdBase t2, 6, sp
    LdBase s0, 7, sp
    LdBase s1, 8, sp
    LdBase a0, 9, sp
    LdBase a1, 10, sp
    LdBase a2, 11, sp
    LdBase a3, 12, sp
    LdBase a4, 13, sp
    LdBase a5, 14, sp
    LdBase a6, 15, sp
    LdBase a7, 16, sp
    LdBase s2, 17, sp
    LdBase s3, 18, sp
    LdBase s4, 19, sp
    LdBase s5, 20, sp
    LdBase s6, 21, sp
    LdBase s7, 22, sp
    LdBase s8, 23, sp
    LdBase s9, 24, sp
    LdBase s10, 25, sp
    LdBase s11, 26, sp
    LdBase t3, 27, sp
    LdBase t4, 28, sp
    LdBase t5, 29, sp
    LdBase t6, 30, sp

    // 恢复 t0
    LdBase t0, 4, sp

    // 恢复 sp
    addi sp, sp, kTrapContextSize
.endm

/*
 * Trap 入口 / 返回
 */

.section .text

.global trap_return
.type trap_return, @function

.global trap_entry
.type trap_entry, @function

.extern HandleTrap

.align 4

trap_entry:
    // 检查是从用户态还是内核态进入
    // 如果是从用户态进入，sscratch 保存的是内核栈地址
    // 如果是从内核态进入，sscratch 为 0
    csrrw sp, sscratch, sp
    bnez sp, .Ltrap_from_user

    // 从内核态进入，恢复 sp，此时 sscratch 变回 0
    csrrw sp, sscratch, sp
    j .Lsave_context

.Ltrap_from_user:
    // 从用户态进入，此时 sp 为内核栈，sscratch 为用户栈 sp

.Lsave_context:
    // 保存 Trap 上下文
    SaveTrapContext

    // 检查 sscratch，如果非 0 说明是从用户态进入，Context 中的 sp (offset 1) 保存的是错误的（内核栈地址）
    // 需要将 sscratch（用户 sp）保存到 Context 中
    csrr t0, sscratch
    beqz t0, .Lcall_handler

    // 覆盖 Context 中的 sp
    sd t0, 8(sp)
    // 将 sscratch 清零，保证在内核态执行时 sscratch 为 0
    csrw sscratch, x0

.Lcall_handler:
    // 将栈指针 sp (指向 TrapContext) 作为参数传递给 HandleTrap
    mv a0, sp
    call HandleTrap

trap_return:
    // 1. kernel_thread_entry 跳转到这里时，a0 是参数 (Context*)
    // 2. 将 a0 赋值给 sp，切换到 Trap 上下文所在的栈位置
    mv sp, a0

    // 恢复 Trap 上下文
    // 检查 sstatus.SPP (offset 31) 判断返回到用户态还是内核态
    ld t0, 31*8(sp)
    andi t0, t0, 0x100
    bnez t0, .Lret_to_kernel

.Lret_to_user:
    // 返回用户态
    // 1. 取出用户栈 sp 到 t0
    ld t0, 8(sp)
    // 2. 将用户栈 sp 保存到 sscratch
    csrw sscratch, t0
    // 3. 恢复通用寄存器 (sp 此时指向内核栈顶)
    RestoreTrapContext
    // 4. 交换 sp and sscratch
    // 此时 sp = 用户栈, sscratch = 内核栈顶
    csrrw sp, sscratch, sp
    sret

.Lret_to_kernel:
    // 返回内核态
    RestoreTrapContext
    // 从 S 模式异常中返回
    sret
