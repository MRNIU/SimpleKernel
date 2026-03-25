/// AArch64 核间中断（IPI）支持
///
/// - SGI：通过 GICv3 `ICC_SGI1R_EL1` 发送软件生成中断
/// - SMP：通过 `arm-psci` crate 构造 PSCI CPU_ON 调用唤醒从核
use arm_psci::{EntryPoint, Function, Mpidr};

// _boot 入口点（boot.S 中定义）
// 从核必须经过 _boot 而非 _start，因为 _boot 负责：
//   1. 读取 MPIDR_EL1 获取 core ID
//   2. 按 core ID 设置 per-core 栈（sp）
// SAFETY: 链接器保证该符号存在于内核镜像中
unsafe extern "C" {
    fn _boot();
}

/// 向指定 CPU 发送 IPI（使用 GICv3 SGI 0）
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

/// 通过 HVC 执行 PSCI 调用，返回 x0（ReturnCode）
///
/// # Safety
/// HVC 是 EL1→EL2 特权转换指令。调用方必须确保 `regs` 的内容构成合法的 PSCI 请求。
unsafe fn psci_hvc_call(regs: &[u64; 4]) -> i64 {
    let ret: u64;
    // SAFETY: HVC 是标准固件接口入口；regs 由 arm-psci crate 构造，保证格式合法
    unsafe {
        core::arch::asm!(
            "hvc #0",
            inout("x0") regs[0] => ret,
            in("x1") regs[1],
            in("x2") regs[2],
            in("x3") regs[3],
            out("x4") _,
            out("x5") _,
            out("x6") _,
            out("x7") _,
        );
    }
    ret as i64
}

/// 启动所有从核
///
/// 使用 `arm-psci` crate 构造 `Function::CpuOn` 请求，通过 HVC 发送给固件。
/// 每个从核以 `_boot` 为入口（初始化栈后跳转到 `_start`），context_id = 0。
pub fn wake_up_other_cores() {
    let core_count = crate::per_cpu::BASIC_INFO
        .get()
        .map(|info| info.core_count)
        .unwrap_or(1);

    if core_count <= 1 {
        log::info!("SMP: 单核模式，无从核需要启动");
        return;
    }

    // SAFETY: _boot 由链接器定义，地址在内核镜像生命周期内有效
    // 先转为函数指针类型再转为 usize（Rust 2024 不允许函数 item 直接转 usize）
    let entry_addr = _boot as unsafe extern "C" fn() as usize as u64;

    for cpu_id in 1..core_count {
        // 构造 PSCI CPU_ON 64-bit 请求
        let target_cpu = Mpidr {
            aff0: cpu_id as u8,
            aff1: 0,
            aff2: 0,
            aff3: Some(0), // 64-bit 模式（CpuOn64 = 0xC400_0003）
        };
        let func = Function::CpuOn {
            target_cpu,
            entry: EntryPoint::Entry64 {
                entry_point_address: entry_addr,
                context_id: 0,
            },
        };

        let mut regs = [0u64; 4];
        func.copy_to_array(&mut regs);

        // SAFETY: regs 由 arm-psci 构造，格式合法
        let ret = unsafe { psci_hvc_call(&regs) };

        if ret == 0 {
            log::info!("SMP: cpu {} 启动成功 (entry=0x{:x})", cpu_id, entry_addr);
        } else {
            log::warn!("SMP: cpu {} 启动失败 (psci_ret={})", cpu_id, ret);
        }
    }
}
