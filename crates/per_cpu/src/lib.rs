// Copyright The SimpleKernel Contributors

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
//! 3. per-CPU 基地址寄存器指向当前 CPU 的副本
//! 4. 访问：`base + (模板地址 - __percpu_start)` = 当前 CPU 的变量地址

#![no_std]
#![feature(sync_unsafe_cell)]

// 让 #[cpu_local] 宏展开的 `per_cpu::CpuLocal` 路径在本 crate 内部也能解析
extern crate self as per_cpu;

use core::sync::atomic::{AtomicBool, Ordering};

use config::{MAX_CORE_COUNT, PERCPU_AREA_MAX};

pub use macros::cpu_local;

fn assert_initialized(api: &str) {
    if !PERCPU_INITIALIZED.load(Ordering::Acquire) {
        panic!(
            "per_cpu: {api} 在 percpu_init() 前被调用: raw_core_id={}, MAX_CORE_COUNT={}",
            arch_primitives::core_id(),
            MAX_CORE_COUNT
        );
    }
}

fn assert_core_id_in_range(core_id: usize, context: &str) {
    assert!(
        core_id < MAX_CORE_COUNT,
        "per_cpu: {context} core_id {} 超出 MAX_CORE_COUNT {}",
        core_id,
        MAX_CORE_COUNT
    );
}

fn assert_offset_in_range<T>(offset: usize, context: &str) {
    let size = core::mem::size_of::<T>();
    let end = offset.checked_add(size).unwrap_or_else(|| {
        panic!("per_cpu: {context} 偏移溢出: offset={offset:#x}, size={size:#x}")
    });
    assert!(
        end <= PERCPU_AREA_MAX,
        "per_cpu: {context} 访问超出 per-CPU 区域: offset={offset:#x}, size={size:#x}, area_size={PERCPU_AREA_MAX:#x}"
    );
}

// SAFETY: 链接脚本提供 `.percpu` section 的起止符号；本 crate 只读取其地址
// 计算模板偏移，不解引用未知外部内存。
unsafe extern "C" {
    static __percpu_start: u8;
    static __percpu_end: u8;
}

/// Per-CPU 变量的包装器。
///
/// 数据在 `.percpu` section 的模板中，初始化后每 CPU 各有一份副本，
/// 通过基地址寄存器 + 偏移量访问。
pub struct CpuLocal<T: Sync> {
    /// 指向模板变量的指针。
    template_ptr: *const T,
}

// SAFETY: CpuLocal 只是偏移计算器，T: Sync 保证跨线程共享安全。
unsafe impl<T: Sync> Send for CpuLocal<T> {}
// SAFETY: CpuLocal 不提供全局共享可变引用；每次访问都会按当前或目标核心计算独立副本地址。
unsafe impl<T: Sync> Sync for CpuLocal<T> {}

impl<T: Sync> CpuLocal<T> {
    /// 由 `#[cpu_local]` 宏调用，不应手动使用。
    ///
    /// # Safety
    /// `ptr` 必须指向 `.percpu` section 中由宏生成的 static 变量。
    #[doc(hidden)]
    pub const unsafe fn __new(ptr: *const T) -> Self {
        Self { template_ptr: ptr }
    }

    /// 计算当前变量在 per-CPU 区域内的偏移量。
    #[inline(always)]
    fn offset(&self) -> usize {
        // SAFETY: 链接器保证 __percpu_start 指向 .percpu section 起始
        self.template_ptr as usize - unsafe { &__percpu_start as *const u8 as usize }
    }

    /// 获取当前 CPU 的变量的不可变引用。
    ///
    /// 对于 `AtomicBool` 等原子类型，无需关中断即可安全调用。
    /// 对于非原子类型，调用方应确保中断已关闭。
    ///
    /// # Panics
    ///
    /// per-CPU 子系统尚未初始化、当前 per-CPU base 为空，或模板偏移超出保留区域时 panic。
    #[inline(always)]
    pub fn get(&self) -> &T {
        assert_initialized("CpuLocal::get");
        let offset = self.offset();
        assert_offset_in_range::<T>(offset, "CpuLocal::get");
        let base = arch_primitives::percpu_base();
        assert!(
            base != 0,
            "per_cpu: CpuLocal::get 读取到空 per-CPU base: offset={offset:#x}, size={:#x}",
            core::mem::size_of::<T>()
        );
        // SAFETY: base 指向当前 CPU 的 per-CPU 区域，offset 在范围内
        unsafe { &*((base + offset) as *const T) }
    }

    /// 获取当前 CPU 的变量的可变引用。
    ///
    /// # Safety
    ///
    /// 调用方必须确保无并发访问（通常通过关中断保证）。
    ///
    /// # Panics
    ///
    /// per-CPU 子系统尚未初始化、当前 per-CPU base 为空，或模板偏移超出保留区域时 panic。
    #[inline(always)]
    #[expect(
        clippy::mut_from_ref,
        reason = "per-CPU 内部可变性：每个 CPU 拥有独立副本"
    )]
    pub unsafe fn get_mut(&self) -> &mut T {
        assert_initialized("CpuLocal::get_mut");
        let offset = self.offset();
        assert_offset_in_range::<T>(offset, "CpuLocal::get_mut");
        let base = arch_primitives::percpu_base();
        assert!(
            base != 0,
            "per_cpu: CpuLocal::get_mut 读取到空 per-CPU base: offset={offset:#x}, size={:#x}",
            core::mem::size_of::<T>()
        );
        // SAFETY: 调用方保证无并发访问
        unsafe { &mut *((base + offset) as *mut T) }
    }

    /// 访问指定 CPU 的变量副本（例如通过 IPI 设置其他核心的标志）。
    ///
    /// # Safety
    ///
    /// - 调用方必须确保访问安全（使用原子类型，或目标 CPU 已停止）。
    /// - `target_core` 必须 < `MAX_CORE_COUNT`。
    ///
    /// # Panics
    ///
    /// per-CPU 子系统尚未初始化、`target_core` 越界、目标 base 为空，
    /// 或模板偏移超出保留区域时 panic。
    #[inline(always)]
    pub unsafe fn get_on(&self, target_core: usize) -> &T {
        assert_initialized("CpuLocal::get_on");
        assert_core_id_in_range(target_core, "CpuLocal::get_on target_core");
        let offset = self.offset();
        assert_offset_in_range::<T>(offset, "CpuLocal::get_on");
        // SAFETY: percpu_init() 已填充 PERCPU_BASES
        let bases = unsafe { &*PERCPU_BASES.get() };
        let base = bases[target_core];
        assert!(
            base != 0,
            "per_cpu: CpuLocal::get_on 目标核心 base 未初始化: target_core={target_core}, offset={offset:#x}, size={:#x}",
            core::mem::size_of::<T>()
        );
        // SAFETY: 调用方保证访问安全
        unsafe { &*((base + offset) as *const T) }
    }
}

/// BSS 中为每个 CPU 预留的 per-CPU 区域。
/// `percpu_init()` 将 `.percpu` 模板复制到每个槽位。
#[repr(C, align(128))]
struct PerCpuArea {
    data: [u8; PERCPU_AREA_MAX],
}

// SAFETY: PerCpuArea 仅在 percpu_init()（单核、中断关闭）中写入，
// 之后各核心只读自己的槽位，不存在数据竞争。
unsafe impl Sync for PerCpuArea {}

static PERCPU_AREAS: core::cell::SyncUnsafeCell<[PerCpuArea; MAX_CORE_COUNT]> =
    core::cell::SyncUnsafeCell::new(
        [const {
            PerCpuArea {
                data: [0u8; PERCPU_AREA_MAX],
            }
        }; MAX_CORE_COUNT],
    );

/// 每个 CPU 的 per-CPU 区域基地址，`percpu_init()` 填充。
static PERCPU_BASES: core::cell::SyncUnsafeCell<[usize; MAX_CORE_COUNT]> =
    core::cell::SyncUnsafeCell::new([0; MAX_CORE_COUNT]);

/// per-CPU 系统是否已初始化。
/// 公开访问入口在该标志发布前必须 fail-fast；初始化代码自身直接读取硬件核心 ID。
static PERCPU_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// 当前核心 ID（由 `percpu_init()` 写入每个 CPU 的区域）。
#[cpu_local]
static CORE_ID: usize = 0;

/// 主核 per-CPU 初始化——复制模板、设置每个 CPU 的基地址寄存器。
///
/// 必须在任何 `#[cpu_local]` 访问之前调用（`logging::init()` 之后）。
///
/// # Safety
///
/// - 只能由主核调用一次
/// - 调用前基地址寄存器必须持有当前核心 ID（riscv64: TP）或为 0（aarch64: TPIDR_EL1）
///
/// # Panics
///
/// 重复初始化、当前核心 ID 越界、`.percpu` 模板超过保留区域，或 `CORE_ID`
/// 模板偏移超出保留区域时 panic。
pub unsafe fn percpu_init() {
    if PERCPU_INITIALIZED.load(Ordering::Acquire) {
        panic!(
            "per_cpu: percpu_init() 被重复调用: current_core_id={}, MAX_CORE_COUNT={}",
            current_core_id(),
            MAX_CORE_COUNT
        );
    }

    // SAFETY: 链接器保证 `__percpu_start` 指向 `.percpu` section 起始符号。
    let template = unsafe { &__percpu_start as *const u8 };
    // SAFETY: 链接器保证 `__percpu_end` 指向 `.percpu` section 结束符号。
    let template_end = unsafe { &__percpu_end as *const u8 };
    let template_size = template_end as usize - template as usize;
    assert!(
        template_size <= PERCPU_AREA_MAX,
        "per_cpu: .percpu 模板超出 per-CPU 区域: template_start={:p}, template_end={:p}, template_size={template_size:#x}, area_size={PERCPU_AREA_MAX:#x}",
        template,
        template_end
    );

    let my_core_id = arch_primitives::core_id();
    assert_core_id_in_range(my_core_id, "percpu_init current core");

    // SAFETY: 单核调用，中断关闭，独占访问 per-CPU storage。
    let areas = unsafe { &mut *PERCPU_AREAS.get() };
    // SAFETY: 单核调用，中断关闭，独占访问 per-CPU base 表。
    let bases = unsafe { &mut *PERCPU_BASES.get() };
    // SAFETY: `_PERCPU_CORE_ID_RAW` 与 `__percpu_start` 都位于 `.percpu` 模板内，只计算偏移。
    let core_id_offset = unsafe {
        &_PERCPU_CORE_ID_RAW as *const usize as usize - &__percpu_start as *const u8 as usize
    };
    assert_offset_in_range::<usize>(core_id_offset, "CORE_ID");

    // 复制模板到每个 CPU 的区域，并设置 CORE_ID
    for i in 0..MAX_CORE_COUNT {
        let dest = areas[i].data.as_mut_ptr();
        // SAFETY: 模板和目标不重叠，大小在范围内
        unsafe { core::ptr::copy_nonoverlapping(template, dest, template_size) };
        bases[i] = dest as usize;

        // 将 CORE_ID 写入每个 CPU 的区域
        // SAFETY: 偏移在 per-CPU 区域范围内
        unsafe { *((dest as usize + core_id_offset) as *mut usize) = i };
    }

    // 设置当前核的 per-CPU 基地址寄存器
    // SAFETY: bases 已正确初始化
    unsafe { arch_primitives::set_percpu_base(bases[my_core_id]) };

    PERCPU_INITIALIZED.store(true, Ordering::Release);
}

/// 从核 per-CPU 初始化——设置 per-CPU 基地址寄存器指向该核的区域。
///
/// 内部通过 [`arch_primitives::core_id()`] 直接读取硬件寄存器确定当前核心 ID，
/// 不依赖 per-CPU 变量。调用完成后 `current_core_id()` 即可正常工作。
///
/// # Safety
///
/// - `percpu_init()` 必须已由主核调用完成
///
/// # Panics
///
/// per-CPU 子系统尚未初始化、当前核心 ID 越界，或目标核心 base 未初始化时 panic。
pub unsafe fn percpu_init_smp() {
    assert_initialized("percpu_init_smp");
    let id = arch_primitives::core_id();
    assert_core_id_in_range(id, "percpu_init_smp current core");
    // SAFETY: percpu_init() 已填充 PERCPU_BASES，id 来自硬件寄存器
    let bases = unsafe { &*PERCPU_BASES.get() };
    assert!(
        bases[id] != 0,
        "per_cpu: percpu_init_smp 目标核心 base 未初始化: core_id={id}, MAX_CORE_COUNT={MAX_CORE_COUNT}"
    );
    // SAFETY: bases[id] 已在 percpu_init() 中正确初始化
    unsafe { arch_primitives::set_percpu_base(bases[id]) };
}

/// 读取当前核心 ID。
///
/// # Panics
/// 当 per-CPU 子系统尚未初始化，或当前 core id 超出 [`MAX_CORE_COUNT`] 时 panic。
#[inline(always)]
pub fn current_core_id() -> usize {
    assert_initialized("current_core_id");
    let core_id = *CORE_ID.get();
    assert_core_id_in_range(core_id, "current_core_id");
    core_id
}
