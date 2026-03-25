/// RISC-V 64 核间中断（IPI）支持
///
/// 通过 SBI legacy send_ipi 发送软件中断，通过 PSCI（hart_start）唤醒从核。
use super::context::TrapContext;

// _start 入口点（boot.S 中定义）
// SAFETY: 链接器保证该符号存在于内核镜像中
unsafe extern "C" {
    fn _start(argc: i32, argv: *const *const u8);
}

/// 向指定 hart 发送 IPI（S 模式软件中断）
///
/// # 参数
/// - `hart_id`：目标 hart 的 ID
pub fn send_ipi(hart_id: usize) {
    // HartMask::from_mask_base(mask=1, base=hart_id) 表示精确指定单个 hart
    let mask = sbi_rt::HartMask::from_mask_base(1, hart_id);
    sbi_rt::send_ipi(mask).ok();
    log::info!("IPI sent to hart {}", hart_id);
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
    let core_id = crate::per_cpu::current_core_id();
    log::info!("IPI received on core {}", core_id);
}

/// 启动所有从核
///
/// 读取 BASIC_INFO 中的 core_count，对每个从核（hart 1..core_count）调用
/// SBI hart_start，传入 `_start` 作为入口，hart_id 作为 argc，0 作为 argv。
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
    // 先转为函数指针类型再转为 usize（Rust 2024 不允许函数 item 直接转 usize）
    let start_addr = _start as unsafe extern "C" fn(i32, *const *const u8) as usize;

    for hart_id in 1..core_count {
        let ret = sbi_rt::hart_start(hart_id, start_addr, 0);
        if ret.is_ok() {
            log::info!("SMP: hart {} 启动成功 (entry=0x{:x})", hart_id, start_addr);
        } else {
            log::warn!(
                "SMP: hart {} 启动失败 (error={}, value={})",
                hart_id,
                ret.error,
                ret.value
            );
        }
    }
}
