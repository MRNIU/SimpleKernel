/// AArch64 核间中断（IPI）支持
///
/// 通过 ICC_SGI1R_EL1 发送 SGI（软件生成中断），
/// 通过 PSCI CPU_ON HVC 调用唤醒从核。

// _start 入口点（boot.S 中定义）
// SAFETY: 链接器保证该符号存在于内核镜像中
unsafe extern "C" {
    fn _start(argc: i32, argv: *const *const u8);
}

/// 向指定 CPU 发送 IPI（使用 GICv3 SGI 1）
///
/// 通过写 ICC_SGI1R_EL1 触发 SGI，TargetList = (1 << cpu_id)，INTID = 0。
///
/// # 参数
/// - `cpu_id`：目标 CPU 的 MPIDR Aff0 字段值（通常等于 CPU 编号）
pub fn send_ipi(cpu_id: usize) {
    // ICC_SGI1R_EL1 编码：
    //   [15:0]  TargetList  — 目标 CPU 位掩码
    //   [27:24] INTID       — SGI 编号（0–15）
    //   其余位为 0
    let target_list: u64 = 1u64 << (cpu_id & 0xF);
    let sgi_value: u64 = target_list; // INTID=0，其余位=0
    // SAFETY: ICC_SGI1R_EL1 在 EL1 下可写（GICv3 CPU 接口使能后）
    unsafe {
        core::arch::asm!(
            "msr icc_sgi1r_el1, {v}",
            "isb",
            v = in(reg) sgi_value,
        );
    }
    log::info!("IPI sent to cpu {}", cpu_id);
}

/// 启动所有从核
///
/// 通过 PSCI CPU_ON HVC 调用（功能号 0xC400_0003）启动从核。
/// 每个从核以 `_start` 为入口，MPIDR 作为参数。
pub fn wake_up_other_cores() {
    let core_count = crate::per_cpu::BASIC_INFO
        .get()
        .map(|info| info.core_count)
        .unwrap_or(1);

    if core_count <= 1 {
        log::info!("SMP: 单核模式，无从核需要启动");
        return;
    }

    // SAFETY: _start 由链接器定义，地址在内核镜像生命周期内有效
    let entry = _start as usize as u64;

    for cpu_id in 1..core_count {
        // PSCI CPU_ON (64-bit): 功能号 0xC400_0003
        // x0 = PSCI_CPU_ON, x1 = target_cpu (MPIDR), x2 = entry, x3 = context_id
        let mpidr = cpu_id as u64; // 简化：MPIDR Aff0 = cpu_id
        let mut ret: u64;
        // SAFETY: HVC 是特权级转换指令，在 EL1 下有效；PSCI CPU_ON 是标准固件接口
        unsafe {
            core::arch::asm!(
                "hvc #0",
                inout("x0") 0xC400_0003u64 => ret,
                in("x1") mpidr,
                in("x2") entry,
                in("x3") 0u64,
                // x4–x17 可能被 HVC 破坏，标记为 clobber
                out("x4") _,
                out("x5") _,
                out("x6") _,
                out("x7") _,
            );
        }
        if ret == 0 {
            log::info!("SMP: cpu {} 启动成功 (entry=0x{:x})", cpu_id, entry);
        } else {
            log::warn!("SMP: cpu {} 启动失败 (psci_ret={})", cpu_id, ret as i64);
        }
    }
}
