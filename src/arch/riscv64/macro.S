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
 * @brief 中断/异常上下文切换所需保存的寄存器数量
 *
 * 31 个通用寄存器(x1-x31) + 4 个 CSR + 32 个浮点寄存器(f0-f31) + 1 个 fcsr
 * 总计: 31 + 4 + 32 + 1 = 68
 */
.equ kTrapContextRegsCount, 68

/**
 * @brief 中断/异常上下文切换所需保存的寄存器大小
 * 68 个寄存器 * 8 字节 = 544 字节
 */
.equ kTrapContextSize, kTrapContextRegsCount * kRegsBytes

/**
 * @brief 函数调用上下文切换所需保存的寄存器数量 (Callee-saved)
 * ra, s0-s11 (13 regs) + fs0-fs11 (12 regs) = 25 regs
 */
.equ kCalleeSavedContextRegsCount, 25

/**
 * @brief 函数调用上下文切换所需保存的寄存器大小 (Callee-saved)
 * 25 * 8 = 200 bytes.
 */
.equ kCalleeSavedContextSize, kCalleeSavedContextRegsCount * kRegsBytes

/**
 * @brief 将寄存器 a 保存在 c 偏移 b 的位置
 * @param a 源寄存器
 * @param b 偏移量 (乘以 kRegsBytes)
 * @param c 基地址寄存器
 */
.macro SdBase a, b, c
    sd \a, ((\b) * kRegsBytes)(\c)
.endm

/**
 * @brief 从 c 的偏移 b 处获取数据并赋值给寄存器 a
 * @param a 目标寄存器
 * @param b 偏移量 (乘以 kRegsBytes)
 * @param c 基地址寄存器
 */
.macro LdBase a, b, c
    ld \a, ((\b) * kRegsBytes)(\c)
.endm

/**
 * @brief 将 float 寄存器 a 保存在 c 偏移 b 的位置
 * @param a 源浮点寄存器
 * @param b 偏移量 (乘以 kRegsBytes)
 * @param c 基地址寄存器
 */
.macro FsdBase a, b, c
    fsd \a, ((\b) * kRegsBytes)(\c)
.endm

/**
 * @brief 从 c 的偏移 b 处获取数据并赋值给 float 寄存器 a
 * @param a 目标浮点寄存器
 * @param b 偏移量 (乘以 kRegsBytes)
 * @param c 基地址寄存器
 */
.macro FldBase a, b, c
    fld \a, ((\b) * kRegsBytes)(\c)
.endm

/**
 * @brief 保存中断/异常上下文 (TrapContext)
 *
 * 保存所有通用寄存器 (x1-x31) 和必要的 CSR 到栈上。
 *
 * @note 此宏会自动分配栈空间 (addi sp, sp, -kTrapContextSize)。
 * @note 保存的 sp 是分配空间之前的 sp。
 * @note 如果是从用户态陷入，通常需要手动将 sscratch (用户 sp) 覆盖到 offset 1。
 */
.macro SaveTrapContext
    addi sp, sp, -ALIGN_16(kTrapContextSize)

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

    // 保存浮点寄存器
    FsdBase ft0, 31, sp
    FsdBase ft1, 32, sp
    FsdBase ft2, 33, sp
    FsdBase ft3, 34, sp
    FsdBase ft4, 35, sp
    FsdBase ft5, 36, sp
    FsdBase ft6, 37, sp
    FsdBase ft7, 38, sp
    FsdBase fs0, 39, sp
    FsdBase fs1, 40, sp
    FsdBase fa0, 41, sp
    FsdBase fa1, 42, sp
    FsdBase fa2, 43, sp
    FsdBase fa3, 44, sp
    FsdBase fa4, 45, sp
    FsdBase fa5, 46, sp
    FsdBase fa6, 47, sp
    FsdBase fa7, 48, sp
    FsdBase fs2, 49, sp
    FsdBase fs3, 50, sp
    FsdBase fs4, 51, sp
    FsdBase fs5, 52, sp
    FsdBase fs6, 53, sp
    FsdBase fs7, 54, sp
    FsdBase fs8, 55, sp
    FsdBase fs9, 56, sp
    FsdBase fs10, 57, sp
    FsdBase fs11, 58, sp
    FsdBase ft8, 59, sp
    FsdBase ft9, 60, sp
    FsdBase ft10, 61, sp
    FsdBase ft11, 62, sp

    // 保存 fcsr
    frcsr t0
    SdBase t0, 63, sp

    // 保存 CSRs (使用 t0 作为临时寄存器，t0 原值已 preservation at offset 4)
    csrr t0, sstatus
    SdBase t0, 64, sp
    csrr t0, sepc
    SdBase t0, 65, sp
    csrr t0, stval
    SdBase t0, 66, sp
    csrr t0, scause
    SdBase t0, 67, sp
.endm

/**
 * @brief 恢复中断/异常上下文 (TrapContext)
 *
 * 从栈上恢复所有通用寄存器 (x1-x31) 和 CSR。
 *
 * @note 此宏会恢复 sp，等于隐式释放栈空间 (若 TrapContext 中的 sp 是分配前的栈顶)。
 */
.macro RestoreTrapContext
    // 恢复 CSRs (使用 t0 作为临时寄存器)
    LdBase t0, 64, sp
    csrw sstatus, t0
    LdBase t0, 65, sp
    csrw sepc, t0
    LdBase t0, 66, sp
    csrw stval, t0
    LdBase t0, 67, sp
    csrw scause, t0

    // 恢复 fcsr
    LdBase t0, 63, sp
    fscsr t0

    // 恢复浮点寄存器
    FldBase ft0, 31, sp
    FldBase ft1, 32, sp
    FldBase ft2, 33, sp
    FldBase ft3, 34, sp
    FldBase ft4, 35, sp
    FldBase ft5, 36, sp
    FldBase ft6, 37, sp
    FldBase ft7, 38, sp
    FldBase fs0, 39, sp
    FldBase fs1, 40, sp
    FldBase fa0, 41, sp
    FldBase fa1, 42, sp
    FldBase fa2, 43, sp
    FldBase fa3, 44, sp
    FldBase fa4, 45, sp
    FldBase fa5, 46, sp
    FldBase fa6, 47, sp
    FldBase fa7, 48, sp
    FldBase fs2, 49, sp
    FldBase fs3, 50, sp
    FldBase fs4, 51, sp
    FldBase fs5, 52, sp
    FldBase fs6, 53, sp
    FldBase fs7, 54, sp
    FldBase fs8, 55, sp
    FldBase fs9, 56, sp
    FldBase fs10, 57, sp
    FldBase fs11, 58, sp
    FldBase ft8, 59, sp
    FldBase ft9, 60, sp
    FldBase ft10, 61, sp
    FldBase ft11, 62, sp

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
    addi sp, sp, ALIGN_16(kTrapContextSize)
.endm

/**
 * @brief 保存 Callee-saved 寄存器
 * 保存 ra, s0-s11, fs0-fs11
 */
.macro SaveCalleeSavedContext
    addi sp, sp, -ALIGN_16(kCalleeSavedContextSize)

    SdBase ra, 0, sp
    SdBase s0, 1, sp
    SdBase s1, 2, sp
    SdBase s2, 3, sp
    SdBase s3, 4, sp
    SdBase s4, 5, sp
    SdBase s5, 6, sp
    SdBase s6, 7, sp
    SdBase s7, 8, sp
    SdBase s8, 9, sp
    SdBase s9, 10, sp
    SdBase s10, 11, sp
    SdBase s11, 12, sp

    FsdBase fs0, 13, sp
    FsdBase fs1, 14, sp
    FsdBase fs2, 15, sp
    FsdBase fs3, 16, sp
    FsdBase fs4, 17, sp
    FsdBase fs5, 18, sp
    FsdBase fs6, 19, sp
    FsdBase fs7, 20, sp
    FsdBase fs8, 21, sp
    FsdBase fs9, 22, sp
    FsdBase fs10, 23, sp
    FsdBase fs11, 24, sp
.endm

/**
 * @brief 恢复 Callee-saved 寄存器
 * 恢复 ra, s0-s11, fs0-fs11
 */
.macro RestoreCalleeSavedContext
    LdBase ra, 0, sp
    LdBase s0, 1, sp
    LdBase s1, 2, sp
    LdBase s2, 3, sp
    LdBase s3, 4, sp
    LdBase s4, 5, sp
    LdBase s5, 6, sp
    LdBase s6, 7, sp
    LdBase s7, 8, sp
    LdBase s8, 9, sp
    LdBase s9, 10, sp
    LdBase s10, 11, sp
    LdBase s11, 12, sp

    FldBase fs0, 13, sp
    FldBase fs1, 14, sp
    FldBase fs2, 15, sp
    FldBase fs3, 16, sp
    FldBase fs4, 17, sp
    FldBase fs5, 18, sp
    FldBase fs6, 19, sp
    FldBase fs7, 20, sp
    FldBase fs8, 21, sp
    FldBase fs9, 22, sp
    FldBase fs10, 23, sp
    FldBase fs11, 24, sp

    addi sp, sp, ALIGN_16(kCalleeSavedContextSize)
.endm
