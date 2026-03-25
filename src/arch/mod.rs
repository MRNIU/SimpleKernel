#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;

#[cfg(target_arch = "riscv64")]
pub(crate) mod riscv64;

/// 架构操作契约——所有架构相关的函数。
///
/// 所有方法均为关联函数（无 `self`），因为架构操作是全局的、无状态的。
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

    /// 从引导参数中提取从核 core ID
    fn secondary_core_id(argc: i32, argv: *const *const u8) -> usize;

    /// 读取当前核心 ID
    fn core_id() -> usize;

    /// 查询中断是否处于使能状态
    fn irq_enabled() -> bool;

    /// 禁用中断
    fn irq_disable();

    /// 使能中断
    ///
    /// # Safety
    /// 调用方必须确保在使能中断后不会违反临界区不变量。
    unsafe fn irq_enable();

    /// 刷新 TLB（在新增页表映射后调用）
    fn flush_tlb();

    /// 映射分页激活前必须就绪的架构特定 MMIO
    fn map_early_mmio(pt: &mut crate::memory::page_table::PageTable) -> crate::error::KResult<()>;

    /// 激活页表（写入 satp / ttbr0_el1 等硬件寄存器）
    ///
    /// # Safety
    /// 调用方必须确保 `pt` 覆盖了激活后将执行的所有代码和数据。
    unsafe fn activate_page_table(pt: &crate::memory::page_table::PageTable);

    /// 向早期控制台输出字符串（SBI putchar / PL011 MMIO）
    fn console_write(s: &str);
}

#[cfg(target_arch = "riscv64")]
pub type Arch = riscv64::Riscv64;

#[cfg(target_arch = "aarch64")]
pub type Arch = aarch64::Aarch64;
