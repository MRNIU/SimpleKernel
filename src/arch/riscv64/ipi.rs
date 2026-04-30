/// RISC-V 64 核间中断（IPI）支持
///
/// 通过 SBI legacy send_ipi 发送软件中断，通过 PSCI（hart_start）唤醒从核。
use super::context::TrapContext;

// _boot 入口点（boot.S 中定义）
// 从核必须经过 _boot 而非 _start，因为 _boot 负责：
//   1. 按 hart_id 设置 per-core 栈（sp）
//   2. 将 hart_id 写入 tp 寄存器（current_core_id() 依赖）
//   3. 初始化 gp 寄存器
// SAFETY: 链接器保证该符号存在于内核镜像中
unsafe extern "C" {
    fn _boot();
}

/// 向指定 hart 发送 IPI（S 模式软件中断）
///
/// # 参数
/// - `hart_id`：目标 hart 的 ID
pub fn send_ipi(hart_id: usize) {
    // HartMask::from_mask_base(mask=1, base=hart_id) 表示精确指定单个 hart
    let mask = sbi_rt::HartMask::from_mask_base(1, hart_id);
    let ret = sbi_rt::send_ipi(mask);
    assert!(
        ret.is_ok(),
        "send_ipi: 发送到 hart {} 失败 (error={}, value={})",
        hart_id,
        ret.error as isize,
        ret.value
    );
    log::debug!("IPI sent to hart {}", hart_id);
}

/// 处理接收到的 IPI
///
/// 清除 SIP.SSIP 位，防止反复触发。
///
/// # 参数
/// - `_ctx`：陷阱上下文（当前未使用，为将来任务唤醒预留）
pub fn handle_ipi(_ctx: &mut TrapContext) {
    // 清除 SSIP（sip 第 1 位，值为 2）
    // SAFETY: csrc 是 CSR 原子清位指令，S 模式下可安全执行
    unsafe {
        core::arch::asm!(
            "csrc sip, {bit}",
            bit = in(reg) 2usize,
        );
    }
    let core_id = per_cpu::current_core_id();
    crate::tlb_shootdown::handle_ipi();
    log::debug!("IPI received on core {}", core_id);
}

/// 启动所有从核
///
/// 读取 `crate::CORE_COUNT`，对每个从核调用 SBI hart_start。
/// 跳过当前核心（主核），因为任何 hart 都可能成为主核（取决于谁先到达 `_start`）。
pub fn wake_secondary_cores() {
    let core_count = crate::CORE_COUNT.get().copied().unwrap_or(1);

    if core_count <= 1 {
        log::info!("SMP: 单核模式，无从核需要启动");
        return;
    }

    let my_hart = per_cpu::current_core_id();

    // SAFETY: _boot 由链接器定义，地址在内核镜像生命周期内有效
    // 先转为函数指针类型再转为 usize（Rust 2024 不允许函数 item 直接转 usize）
    let boot_addr = _boot as unsafe extern "C" fn() as usize;

    for hart_id in 0..core_count {
        // 跳过当前核心（已经在运行）
        if hart_id == my_hart {
            continue;
        }
        // SBI hart_start 传递：a0 = hart_id, a1 = opaque（此处为 0）
        // _boot 利用 a0 设置 tp 和 per-core 栈，然后跳转到 _start
        let ret = sbi_rt::hart_start(hart_id, boot_addr, 0);
        if ret.is_ok() {
            log::info!("SMP: hart {} 启动成功 (entry=0x{:x})", hart_id, boot_addr);
        } else {
            log::warn!(
                "SMP: hart {} 启动失败 (error={}, value={})",
                hart_id,
                ret.error as isize,
                ret.value
            );
        }
    }
}
