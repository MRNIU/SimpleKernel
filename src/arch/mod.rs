#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;

#[cfg(target_arch = "riscv64")]
pub(crate) mod riscv64;

/// 架构操作契约——需要高层依赖（内存管理、硬件初始化）的架构相关函数。
///
/// 所有方法均为关联函数（无 `self`），因为架构操作是全局的、无状态的。
///
/// 底层 CPU 原语（中断控制）在 `sync` crate 中提供，
/// TLB 管理在 `memory` crate 中提供，
/// core ID 在 `per_cpu` crate 中提供，tick 计数在 `tick` crate 中提供，
/// 此 trait 仅定义初始化和硬件配置操作。
pub trait ArchOps {
    /// 从引导参数中提取 DTB（设备树）物理地址
    fn dtb_addr(argc: i32, argv: *const *const u8) -> usize;

    /// 主核中断控制器初始化（PLIC / GIC）
    fn init_interrupt();

    /// 从核中断控制器初始化
    fn init_interrupt_smp();

    /// 主核定时器初始化
    fn init_timer();

    /// 从核定时器初始化
    fn init_timer_smp(core_id: usize);

    /// 唤醒所有从核
    fn wake_secondary_cores();

    /// 映射分页激活前必须就绪的架构特定 MMIO
    fn map_early_mmio();

    /// 激活页表（写入 satp / ttbr0_el1 等硬件寄存器）
    ///
    /// # Safety
    /// 调用方必须确保 `pt` 覆盖了激活后将执行的所有代码和数据。
    unsafe fn activate_page_table(pt: &paging::PageTable);

    /// 向早期控制台输出字符串（SBI putchar / PL011 MMIO）
    fn console_write(s: &str);
}

#[cfg(target_arch = "riscv64")]
pub type Arch = riscv64::Riscv64;

#[cfg(target_arch = "aarch64")]
pub type Arch = aarch64::Aarch64;

/// 被调用者保存上下文——架构无关的统一类型别名
#[cfg(target_arch = "riscv64")]
pub type CalleeSavedContext = riscv64::context::CalleeSavedContext;

/// 被调用者保存上下文——架构无关的统一类型别名
#[cfg(target_arch = "aarch64")]
pub type CalleeSavedContext = aarch64::context::CalleeSavedContext;

#[cfg(target_arch = "riscv64")]
pub use riscv64::switch::switch_to;

#[cfg(target_arch = "aarch64")]
pub use aarch64::switch::switch_to;

/// 宿主机编译（`cargo clippy` / `cargo check`）占位类型，不会在目标架构上使用。
#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct CalleeSavedContext {
    _placeholder: u64,
}

#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
impl CalleeSavedContext {
    /// 宿主机编译占位——实际内核从不在 x86_64 上运行
    pub fn init_for_kernel_thread(&mut self, _kstack_top: usize, _entry: fn(usize), _arg: usize) {}
}

/// 宿主机编译占位——实际内核从不在 x86_64 上运行
#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
pub unsafe extern "C" fn switch_to(
    _prev: *mut CalleeSavedContext,
    _next: *const CalleeSavedContext,
) {
}
