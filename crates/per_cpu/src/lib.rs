//! Per-CPU 数据——通过 `#[cpu_local]` 分散声明，每核心独立副本。
//!
//! 本 crate 只提供 per-CPU 机制（声明、初始化、访问），
//! 不包含任何业务变量——各子系统在自己的模块中用 `#[cpu_local]` 声明。
//!
//! ## Per-CPU 状态
//!
//! | 变量 | 类型 | 说明 |
//! |------|------|------|
//! | `CORE_ID` | `usize` | 当前核心 ID（`percpu_init` 写入） |
//!
//! ## 原理
//!
//! 1. `#[cpu_local]` 将变量放入 `.percpu` ELF section（模板）
//! 2. `percpu_init()` 将模板复制 N 份（每 CPU 一份）到 BSS 预留区
//! 3. TP（riscv64）/ TPIDR_EL1（aarch64）指向当前 CPU 的副本
//! 4. 访问：`TP + (模板地址 - __percpu_start)` = 当前 CPU 的变量地址

#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_os = "none", feature(sync_unsafe_cell))]

// 让 #[cpu_local] 宏展开的 `per_cpu::CpuLocal` 路径在本 crate 内部也能解析
extern crate self as per_cpu;

pub use macros::cpu_local;

#[cfg(target_os = "none")]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "none")]
use config::{MAX_CORE_COUNT, PERCPU_AREA_MAX};

#[cfg(target_os = "none")]
unsafe extern "C" {
    static __percpu_start: u8;
    static __percpu_end: u8;
}

/// BSS 中为每个 CPU 预留的 per-CPU 区域。
/// `percpu_init()` 将 `.percpu` 模板复制到每个槽位。
#[cfg(target_os = "none")]
#[repr(C, align(128))]
struct PerCpuArea {
    data: [u8; PERCPU_AREA_MAX],
}

// SAFETY: PerCpuArea 仅在 percpu_init()（单核、中断关闭）中写入，
// 之后各核心只读自己的槽位，不存在数据竞争。
#[cfg(target_os = "none")]
unsafe impl Sync for PerCpuArea {}

#[cfg(target_os = "none")]
static PERCPU_AREAS: core::cell::SyncUnsafeCell<[PerCpuArea; MAX_CORE_COUNT]> =
    core::cell::SyncUnsafeCell::new(
        [const {
            PerCpuArea {
                data: [0u8; PERCPU_AREA_MAX],
            }
        }; MAX_CORE_COUNT],
    );

/// 每个 CPU 的 per-CPU 区域基地址，`percpu_init()` 填充。
#[cfg(target_os = "none")]
static PERCPU_BASES: core::cell::SyncUnsafeCell<[usize; MAX_CORE_COUNT]> =
    core::cell::SyncUnsafeCell::new([0; MAX_CORE_COUNT]);

/// per-CPU 系统是否已初始化。
/// `core_id()` 在初始化前回退到读原始寄存器。
#[cfg(target_os = "none")]
static PERCPU_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// 读取 TP（riscv64）或 TPIDR_EL1（aarch64）寄存器。
#[cfg(target_os = "none")]
#[inline(always)]
fn read_tp() -> usize {
    let val: usize;
    #[cfg(target_arch = "riscv64")]
    // SAFETY: 读取 TP 寄存器，在 S 模式下始终可读
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) val);
    }
    #[cfg(target_arch = "aarch64")]
    // SAFETY: TPIDR_EL1 在 EL1 下始终可读
    unsafe {
        core::arch::asm!("mrs {}, tpidr_el1", out(reg) val);
    }
    val
}

/// 写入 TP（riscv64）或 TPIDR_EL1（aarch64）寄存器。
#[cfg(target_os = "none")]
#[inline(always)]
unsafe fn write_tp(val: usize) {
    #[cfg(target_arch = "riscv64")]
    // SAFETY: 由调用方保证设置正确的 per-CPU 基地址
    unsafe {
        core::arch::asm!("mv tp, {}", in(reg) val);
    }
    #[cfg(target_arch = "aarch64")]
    // SAFETY: 由调用方保证设置正确的 per-CPU 基地址
    unsafe {
        core::arch::asm!("msr tpidr_el1, {}", in(reg) val);
    }
}

/// 主核 per-CPU 初始化——复制模板、设置每个 CPU 的基地址、设置 TP。
///
/// 必须在任何 `#[cpu_local]` 访问之前调用（`logging::init()` 之后）。
///
/// # Safety
/// - 只能由主核调用一次
/// - 调用前 TP 必须持有当前 hart_id（riscv64）或 TPIDR_EL1 为 0（aarch64）
#[cfg(target_os = "none")]
pub unsafe fn percpu_init() {
    assert!(
        !PERCPU_INITIALIZED.load(Ordering::Acquire),
        "percpu_init() 被重复调用"
    );

    let template = unsafe { &__percpu_start as *const u8 };
    let template_size =
        unsafe { &__percpu_end as *const u8 as usize - &__percpu_start as *const u8 as usize };

    let my_core_id = raw_core_id();

    // SAFETY: 单核调用，中断关闭，独占访问
    let areas = unsafe { &mut *PERCPU_AREAS.get() };
    let bases = unsafe { &mut *PERCPU_BASES.get() };

    // 复制模板到每个 CPU 的区域，并设置 CORE_ID
    for i in 0..MAX_CORE_COUNT {
        let dest = areas[i].data.as_mut_ptr();
        // SAFETY: 模板和目标不重叠，大小在范围内
        unsafe { core::ptr::copy_nonoverlapping(template, dest, template_size) };
        bases[i] = dest as usize;

        // 将 CORE_ID 写入每个 CPU 的区域
        let core_id_offset =
            &_PERCPU_CORE_ID_RAW as *const usize as usize - &__percpu_start as *const u8 as usize;
        // SAFETY: 偏移在 per-CPU 区域范围内
        unsafe { *((dest as usize + core_id_offset) as *mut usize) = i };
    }

    // 设置当前核的 TP
    // SAFETY: bases 已正确初始化
    unsafe { write_tp(bases[my_core_id]) };

    PERCPU_INITIALIZED.store(true, Ordering::Release);
}

/// 从核 per-CPU 初始化——设置 TP 指向该核的 per-CPU 区域。
///
/// 内部通过 `raw_core_id()` 直接读取硬件寄存器确定当前核心 ID
/// （riscv64: TP 寄存器，aarch64: MPIDR_EL1），不依赖 per-CPU 变量。
/// 调用完成后 `current_core_id()` 即可正常工作。
///
/// # Safety
/// - `percpu_init()` 必须已由主核调用完成
#[cfg(target_os = "none")]
pub unsafe fn percpu_init_smp() {
    let id = raw_core_id();
    // SAFETY: percpu_init() 已填充 PERCPU_BASES，id 来自硬件寄存器
    let bases = unsafe { &*PERCPU_BASES.get() };
    unsafe { write_tp(bases[id]) };
}

/// 读取当前核心 ID。
///
/// - 初始化后：从 per-CPU `CORE_ID` 变量读取（通过 TP）
/// - 初始化前：回退到读原始寄存器
/// - 宿主机：线程局部唯一 ID
#[inline(always)]
pub fn current_core_id() -> usize {
    #[cfg(target_os = "none")]
    {
        if PERCPU_INITIALIZED.load(Ordering::Acquire) {
            *CORE_ID.get()
        } else {
            raw_core_id()
        }
    }
    #[cfg(not(target_os = "none"))]
    {
        host_core_id()
    }
}

/// 初始化前读取原始核心 ID（riscv64: TP 寄存器，aarch64: MPIDR_EL1.Aff0）。
#[cfg(target_os = "none")]
#[inline(always)]
fn raw_core_id() -> usize {
    #[cfg(target_arch = "riscv64")]
    {
        let id: usize;
        // SAFETY: boot.S 已将 hart_id 写入 TP
        unsafe { core::arch::asm!("mv {}, tp", out(reg) id) };
        id
    }
    #[cfg(target_arch = "aarch64")]
    {
        use aarch64_cpu::registers::{MPIDR_EL1, Readable};
        MPIDR_EL1.read(MPIDR_EL1::Aff0) as usize
    }
}

/// 宿主机——返回当前线程的 core_id。
///
/// 测试时为每个线程分配唯一 ID，非测试时固定返回 0。
#[cfg(not(target_os = "none"))]
fn host_core_id() -> usize {
    #[cfg(test)]
    {
        use core::sync::atomic::{AtomicUsize, Ordering};
        use std::cell::Cell;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        thread_local! {
            static TID: Cell<usize> =
                Cell::new(COUNTER.fetch_add(1, Ordering::Relaxed));
        }
        TID.with(|id| id.get())
    }
    #[cfg(not(test))]
    {
        0
    }
}

/// Per-CPU 变量的包装器。
///
/// 裸机：数据在 `.percpu` section 的模板中，初始化后每 CPU 各有一份副本，
/// 通过 TP + 偏移量访问。
///
/// 宿主机：直接指向普通 static 变量（仅用于 `cargo check` / `cargo test`）。
pub struct CpuLocal<T: Sync> {
    /// 指向模板变量的指针。
    /// 裸机：运行时计算偏移 `ptr - __percpu_start`。
    /// 宿主机：直接解引用。
    template_ptr: *const T,
}

// SAFETY: CpuLocal 只是偏移计算器，T: Sync 保证跨线程共享安全
unsafe impl<T: Sync> Send for CpuLocal<T> {}
unsafe impl<T: Sync> Sync for CpuLocal<T> {}

impl<T: Sync> CpuLocal<T> {
    /// 由 `#[cpu_local]` 宏调用，不应手动使用。
    ///
    /// # Safety
    /// 裸机：`ptr` 必须指向 `.percpu` section 中由宏生成的 static 变量。
    /// 宿主机：`ptr` 指向普通 static 变量。
    #[doc(hidden)]
    pub const unsafe fn __new(ptr: *const T) -> Self {
        Self { template_ptr: ptr }
    }

    /// 计算当前变量在 per-CPU 区域内的偏移量。
    #[cfg(target_os = "none")]
    #[inline(always)]
    fn offset(&self) -> usize {
        self.template_ptr as usize - unsafe { &__percpu_start as *const u8 as usize }
    }

    /// 获取当前 CPU 的变量的不可变引用。
    ///
    /// 对于 `AtomicBool` 等原子类型，无需关中断即可安全调用。
    /// 对于非原子类型，调用方应确保中断已关闭。
    #[inline(always)]
    pub fn get(&self) -> &T {
        #[cfg(target_os = "none")]
        {
            let base = read_tp();
            // SAFETY: base 指向当前 CPU 的 per-CPU 区域，offset 在范围内
            unsafe { &*((base + self.offset()) as *const T) }
        }
        #[cfg(not(target_os = "none"))]
        {
            // SAFETY: 宿主机上指向普通 static，T: Sync 保证共享引用安全
            unsafe { &*self.template_ptr }
        }
    }

    /// 获取当前 CPU 的变量的可变引用。
    ///
    /// # Safety
    /// 调用方必须确保无并发访问（通常通过关中断保证）。
    #[inline(always)]
    #[allow(clippy::mut_from_ref)] // 故意的 per-CPU 内部可变性：每个 CPU 拥有独立副本
    pub unsafe fn get_mut(&self) -> &mut T {
        #[cfg(target_os = "none")]
        {
            let base = read_tp();
            // SAFETY: 调用方保证无并发访问
            unsafe { &mut *((base + self.offset()) as *mut T) }
        }
        #[cfg(not(target_os = "none"))]
        {
            // SAFETY: 宿主机上调用方须保证单线程访问
            unsafe { &mut *(self.template_ptr as *mut T) }
        }
    }

    /// 访问指定 CPU 的变量副本（例如通过 IPI 设置其他核心的标志）。
    ///
    /// # Safety
    /// - 调用方必须确保访问安全（使用原子类型，或目标 CPU 已停止）。
    /// - `target_core` 必须 < `MAX_CORE_COUNT`。
    #[cfg(target_os = "none")]
    #[inline(always)]
    pub unsafe fn get_on(&self, target_core: usize) -> &T {
        debug_assert!(target_core < MAX_CORE_COUNT, "无效的核心 ID: {target_core}");
        // SAFETY: percpu_init() 已填充 PERCPU_BASES
        let bases = unsafe { &*PERCPU_BASES.get() };
        let base = bases[target_core];
        // SAFETY: 调用方保证访问安全
        unsafe { &*((base + self.offset()) as *const T) }
    }

    /// 宿主机上无法真正跨核访问，返回当前值。
    #[cfg(not(target_os = "none"))]
    #[inline(always)]
    pub unsafe fn get_on(&self, _target_core: usize) -> &T {
        unsafe { &*self.template_ptr }
    }
}

/// 当前核心 ID（由 `percpu_init()` 写入每个 CPU 的区域）。
#[cpu_local]
static CORE_ID: usize = 0;

#[cfg(test)]
mod tests {
    use super::*;

    #[cpu_local]
    static TEST_VAR: u32 = 42;

    /// 宿主机上 CpuLocal::get() 返回初始值
    #[test]
    fn cpu_local_get_returns_initial_value() {
        assert_eq!(*TEST_VAR.get(), 42);
    }

    /// current_core_id() 在宿主机上返回线程唯一 ID
    #[test]
    fn core_id_works_on_host() {
        let id = current_core_id();
        assert!(id < 1024);
    }
}
