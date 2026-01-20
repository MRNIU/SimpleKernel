/**
 * @copyright Copyright The SimpleKernel Contributors
 */

// 对齐到 16 字节
#define ALIGN_16(x) (((x) + 15) & ~15)

/**
 * @brief 寄存器长度，8 字节
 */
.equ kRegsBytes, 8

/**
 * @brief 中断/异常上下文切换所需保存的寄存器数量 (TrapContext)
 *
 * 31 个通用寄存器(x0-x30) + 2 个特殊寄存器(sp, pc) +
 * 32 个浮点寄存器(q0-q31，每个 128 位用 2 个 uint64_t 存储，共 64 个) +
 * 4 个状态寄存器(fpsr, fpcr, esr_el1, spsr_el1)
 * 总计: 31 + 2 + 64 + 4 = 101
 */
.equ kTrapContextRegsCount, 101

/**
 * @brief 中断/异常上下文切换所需保存的寄存器大小
 * 101 个寄存器 * 8 字节 = 808 字节（对齐到 16 字节为 816 字节）
 */
.equ kTrapContextSize, ALIGN_16(kTrapContextRegsCount * kRegsBytes)

/**
 * @brief 函数调用上下文切换所需保存的寄存器数量 (CalleeSavedContext)
 * x19-x30 (12 regs) + sp, pc (2 regs) + q8-q15 (8 * 2 = 16 regs) = 30 regs
 */
.equ kCalleeSavedContextRegsCount, 30

/**
 * @brief 函数调用上下文切换所需保存的寄存器大小 (CalleeSavedContext)
 * 30 * 8 = 240 字节（已对齐到 16 字节）
 */
.equ kCalleeSavedContextSize, ALIGN_16(kCalleeSavedContextRegsCount * kRegsBytes)

/**
 * @brief 保存中断/异常上下文 (TrapContext)
 *
 * 保存所有通用寄存器 (x0-x30)、系统寄存器和浮点寄存器到栈上。
 * 对应 TrapContext 结构体布局。
 *
 * @note 此宏会自动分配栈空间 (sub sp, sp, #kTrapContextSize)。
 * @note 保存的 sp 是分配空间之前的 sp。
 */
.macro SaveTrapContext
    // 分配栈空间 (800 字节，已对齐到 16)
    sub sp, sp, #kTrapContextSize

    // 保存通用寄存器 x0-x30 (31 个寄存器，248 字节)
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
    str x30, [sp, #16 * 15]

    // 保存 sp 和 pc (16 字节)
    // sp: 保存分配前的栈指针
    add x9, sp, #kTrapContextSize
    // pc: 从 elr_el1 读取
    mrs x10, elr_el1
    stp x9, x10, [sp, #16 * 15 + 8]

    // 保存浮点/SIMD寄存器 q0-q31 (32 个 128 位寄存器，512 字节)
    stp q0, q1, [sp, #16 * 17]
    stp q2, q3, [sp, #16 * 19]
    stp q4, q5, [sp, #16 * 21]
    stp q6, q7, [sp, #16 * 23]
    stp q8, q9, [sp, #16 * 25]
    stp q10, q11, [sp, #16 * 27]
    stp q12, q13, [sp, #16 * 29]
    stp q14, q15, [sp, #16 * 31]
    stp q16, q17, [sp, #16 * 33]
    stp q18, q19, [sp, #16 * 35]
    stp q20, q21, [sp, #16 * 37]
    stp q22, q23, [sp, #16 * 39]
    stp q24, q25, [sp, #16 * 41]
    stp q26, q27, [sp, #16 * 43]
    stp q28, q29, [sp, #16 * 45]
    stp q30, q31, [sp, #16 * 47]

    // 保存浮点状态寄存器 fpsr, fpcr, esr_el1, spsr_el1 (32 字节)
    // 使用临时基址避免偏移超限 (stp x 指令偏移最大 504)
    add x11, sp, #16 * 49
    mrs x9, fpsr
    mrs x10, fpcr
    stp x9, x10, [x11]
    mrs x9, esr_el1
    mrs x10, spsr_el1
    stp x9, x10, [x11, #16]
.endm

/**
 * @brief 恢复中断/异常上下文 (TrapContext)
 *
 * 从栈上恢复所有通用寄存器、系统寄存器和浮点寄存器。
 *
 * @note 此宏会恢复 sp，等于隐式释放栈空间。
 */
.macro RestoreTrapContext
    // 恢复浮点状态寄存器和异常状态寄存器
    // 使用临时基址避免偏移超限 (ldp x 指令偏移最大 504)
    add x11, sp, #16 * 49
    ldp x9, x10, [x11]
    msr fpsr, x9
    msr fpcr, x10
    ldp x9, x10, [x11, #16]
    msr esr_el1, x9
    msr spsr_el1, x10

    // 恢复浮点/SIMD寄存器 q0-q31
    ldp q0, q1, [sp, #16 * 17]
    ldp q2, q3, [sp, #16 * 19]
    ldp q4, q5, [sp, #16 * 21]
    ldp q6, q7, [sp, #16 * 23]
    ldp q8, q9, [sp, #16 * 25]
    ldp q10, q11, [sp, #16 * 27]
    ldp q12, q13, [sp, #16 * 29]
    ldp q14, q15, [sp, #16 * 31]
    ldp q16, q17, [sp, #16 * 33]
    ldp q18, q19, [sp, #16 * 35]
    ldp q20, q21, [sp, #16 * 37]
    ldp q22, q23, [sp, #16 * 39]
    ldp q24, q25, [sp, #16 * 41]
    ldp q26, q27, [sp, #16 * 43]
    ldp q28, q29, [sp, #16 * 45]
    ldp q30, q31, [sp, #16 * 47]

    // 恢复 pc 到 elr_el1 (sp 暂时不恢复，最后处理)
    ldp x9, x10, [sp, #16 * 15 + 8]
    msr elr_el1, x10

    // 恢复通用寄存器 x0-x30
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
    ldr x30, [sp, #16 * 15]

    // 恢复 sp (释放栈空间)
    add sp, sp, #kTrapContextSize
.endm

/**
 * @brief 保存 Callee-saved 寄存器到指定地址 (CalleeSavedContext)
 * 保存 x19-x30, sp, pc, q8-q15
 *
 * @param base_reg 基地址寄存器 (保存的目标地址)
 */
.macro SaveCalleeSavedContext base_reg
    // 保存 x19-x30 (12 个寄存器)
    stp x19, x20, [\base_reg, #16 * 0]
    stp x21, x22, [\base_reg, #16 * 1]
    stp x23, x24, [\base_reg, #16 * 2]
    stp x25, x26, [\base_reg, #16 * 3]
    stp x27, x28, [\base_reg, #16 * 4]
    stp x29, x30, [\base_reg, #16 * 5]

    // 保存 sp 和 pc (2 个寄存器)
    mov x9, sp
    mov x10, x30  // pc: 保存返回地址（Link Register）
    stp x9, x10, [\base_reg, #16 * 6]

    // 保存 callee-saved 浮点寄存器 q8-q15 (8 个 128 位寄存器)
    stp q8, q9, [\base_reg, #16 * 7]
    stp q10, q11, [\base_reg, #16 * 9]
    stp q12, q13, [\base_reg, #16 * 11]
    stp q14, q15, [\base_reg, #16 * 13]
.endm

/**
 * @brief 从指定地址恢复 Callee-saved 寄存器 (CalleeSavedContext)
 * 恢复 x19-x30, sp, pc, q8-q15
 *
 * @param base_reg 基地址寄存器 (恢复的源地址)
 */
.macro RestoreCalleeSavedContext base_reg
    // 恢复 callee-saved 浮点寄存器 q8-q15
    ldp q8, q9, [\base_reg, #16 * 7]
    ldp q10, q11, [\base_reg, #16 * 9]
    ldp q12, q13, [\base_reg, #16 * 11]
    ldp q14, q15, [\base_reg, #16 * 13]

    // 恢复 sp 和 pc
    ldp x9, x10, [\base_reg, #16 * 6]
    mov sp, x9

    // 恢复 x19-x30
    ldp x19, x20, [\base_reg, #16 * 0]
    ldp x21, x22, [\base_reg, #16 * 1]
    ldp x23, x24, [\base_reg, #16 * 2]
    ldp x25, x26, [\base_reg, #16 * 3]
    ldp x27, x28, [\base_reg, #16 * 4]
    ldp x29, x30, [\base_reg, #16 * 5]

    // 跳转到保存的 pc
    br x10
.endm
