/**
 * @copyright Copyright The SimpleKernel Contributors
 * @note 需要与 MRNIU/cpu_io/include/aarch64/context.hpp 中的结构体定义保持一致
 */

// 对齐到 16 字节
#define ALIGN_16(x) (((x) + 15) & ~15)

/**
 * @brief 寄存器长度，8 字节
 */
.equ kRegsBytes, 8

/**
 * @brief TrapContext 结构体大小
 */
.equ kTrapContextSize, 896

/**
 * @brief CalleeSavedContext 结构体大小
 */
.equ kCalleeSavedContextSize, 176

/**
 * @brief 保存中断/异常上下文 (TrapContext)
 *
 * 保存所有寄存器到栈上
 * 偏移 0-247: x0-x30
 * 偏移 248-255: _padding0
 * 偏移 256-767: q0-q31
 * 偏移 768-783: fpsr, fpcr
 * 偏移 784-831: _padding1 (6 个 uint64_t)
 * 偏移 832-855: elr_el1, spsr_el1, esr_el1
 * 偏移 856-879: sp_el0, tpidr_el0, ttbr0_el1
 * 偏移 880-895: sp_el1, tpidr_el1
 *
 * @note 此宏会自动分配栈空间 (sub sp, sp, #896)。
 */
.macro SaveTrapContext
    // 分配栈空间 (896 字节)
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
    // 偏移 248-255: _padding0 (不需要显式保存)

    // 保存浮点/SIMD寄存器 q0-q31 (偏移 256-767)
    stp q0, q1, [sp, #256]
    stp q2, q3, [sp, #288]
    stp q4, q5, [sp, #320]
    stp q6, q7, [sp, #352]
    stp q8, q9, [sp, #384]
    stp q10, q11, [sp, #416]
    stp q12, q13, [sp, #448]
    stp q14, q15, [sp, #480]
    stp q16, q17, [sp, #512]
    stp q18, q19, [sp, #544]
    stp q20, q21, [sp, #576]
    stp q22, q23, [sp, #608]
    stp q24, q25, [sp, #640]
    stp q26, q27, [sp, #672]
    stp q28, q29, [sp, #704]
    stp q30, q31, [sp, #736]

    // 保存浮点状态寄存器 (偏移 768-783) 和系统寄存器 (偏移 832-895)
    // 使用临时基址避免偏移超出 stp/ldp 的范围 (-512 到 504)
    add x28, sp, #768
    mrs x9, fpsr
    mrs x10, fpcr
    stp x9, x10, [x28]
    // 偏移 784-831: _padding1 (不需要显式保存)

    // 保存系统寄存器 (偏移 832-895，相对 x28 偏移 64-127)
    mrs x9, elr_el1
    mrs x10, spsr_el1
    mrs x11, esr_el1
    stp x9, x10, [x28, #64]
    str x11, [x28, #80]

    mrs x9, sp_el0
    mrs x10, tpidr_el0
    mrs x11, ttbr0_el1
    stp x9, x10, [x28, #88]
    str x11, [x28, #104]

    // sp_el1: 保存分配前的栈指针
    add x9, sp, #kTrapContextSize
    mrs x10, tpidr_el1
    stp x9, x10, [x28, #112]
.endm

/**
 * @brief 恢复中断/异常上下文 (TrapContext)
 *
 * 从栈上恢复所有寄存器，严格按照 context.hpp 中 TrapContext 的布局。
 *
 * @note 此宏会恢复 sp，等于隐式释放栈空间。
 */
.macro RestoreTrapContext
    // 恢复系统寄存器 (偏移 832-895) 和浮点状态寄存器 (偏移 768-783)
    // 使用临时基址避免偏移超出 ldp 的范围 (-512 到 504)
    add x28, sp, #768

    // 恢复系统寄存器 (偏移 832-895，相对 x28 偏移 64-127)
    ldp x9, x10, [x28, #64]
    ldr x11, [x28, #80]
    msr elr_el1, x9
    msr spsr_el1, x10
    // 注意：不恢复 esr_el1，它是硬件在异常时自动设置的只读寄存器

    ldp x9, x10, [x28, #88]
    ldr x11, [x28, #104]
    msr sp_el0, x9
    msr tpidr_el0, x10
    msr ttbr0_el1, x11

    ldp x9, x10, [x28, #112]
    // sp_el1 最后恢复
    msr tpidr_el1, x10

    // 恢复浮点状态寄存器 (偏移 768-783，相对 x28 偏移 0)
    ldp x9, x10, [x28]
    msr fpsr, x9
    msr fpcr, x10

    // 恢复浮点/SIMD寄存器 q0-q31 (偏移 256-767)
    ldp q0, q1, [sp, #256]
    ldp q2, q3, [sp, #288]
    ldp q4, q5, [sp, #320]
    ldp q6, q7, [sp, #352]
    ldp q8, q9, [sp, #384]
    ldp q10, q11, [sp, #416]
    ldp q12, q13, [sp, #448]
    ldp q14, q15, [sp, #480]
    ldp q16, q17, [sp, #512]
    ldp q18, q19, [sp, #544]
    ldp q20, q21, [sp, #576]
    ldp q22, q23, [sp, #608]
    ldp q24, q25, [sp, #640]
    ldp q26, q27, [sp, #672]
    ldp q28, q29, [sp, #704]
    ldp q30, q31, [sp, #736]

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

/**
 * @brief 保存 Callee-saved 寄存器到指定地址 (CalleeSavedContext)
 *
 * 按照 context.hpp 中 CalleeSavedContext 的布局：
 * 偏移 0-95: x19-x30 (12 regs)
 * 偏移 96-159: d8-d15 (8 regs，注意是 64 位的 d 寄存器，不是 128 位的 q 寄存器)
 * 偏移 160-175: sp, pc (2 regs)
 *
 * @param base_reg 基地址寄存器 (保存的目标地址)
 */
.macro SaveCalleeSavedContext base_reg
    // 保存 x19-x30 (偏移 0-95)
    stp x19, x20, [\base_reg, #0]
    stp x21, x22, [\base_reg, #16]
    stp x23, x24, [\base_reg, #32]
    stp x25, x26, [\base_reg, #48]
    stp x27, x28, [\base_reg, #64]
    stp x29, x30, [\base_reg, #80]

    // 保存 d8-d15 (偏移 96-159，64 位浮点寄存器)
    stp d8, d9, [\base_reg, #96]
    stp d10, d11, [\base_reg, #112]
    stp d12, d13, [\base_reg, #128]
    stp d14, d15, [\base_reg, #144]

    // 保存 sp 和 pc (偏移 160-175)
    mov x9, sp
    mov x10, x30  // pc: 保存返回地址（Link Register）
    stp x9, x10, [\base_reg, #160]
.endm

/**
 * @brief 从指定地址恢复 Callee-saved 寄存器 (CalleeSavedContext)
 *
 * 按照 context.hpp 中 CalleeSavedContext 的布局恢复寄存器。
 *
 * @param base_reg 基地址寄存器 (恢复的源地址)
 */
.macro RestoreCalleeSavedContext base_reg
    // 恢复 d8-d15 (偏移 96-159)
    ldp d8, d9, [\base_reg, #96]
    ldp d10, d11, [\base_reg, #112]
    ldp d12, d13, [\base_reg, #128]
    ldp d14, d15, [\base_reg, #144]

    // 恢复 sp 和 pc (偏移 160-175)
    ldp x9, x10, [\base_reg, #160]
    mov sp, x9

    // 恢复 x19-x30 (偏移 0-95)
    ldp x19, x20, [\base_reg, #0]
    ldp x21, x22, [\base_reg, #16]
    ldp x23, x24, [\base_reg, #32]
    ldp x25, x26, [\base_reg, #48]
    ldp x27, x28, [\base_reg, #64]
    ldp x29, x30, [\base_reg, #80]

    // 跳转到保存的 pc
    br x10
.endm
