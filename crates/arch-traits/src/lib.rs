#![cfg_attr(not(test), no_std)]

//! 架构 CPU 原语——打破 sync ↔ arch ↔ per_cpu 循环依赖。
//!
//! 此 crate 提供最底层的 CPU 操作（core_id、中断控制），
//! 它们是简单的 inline asm 指令，无需依赖上层模块。
//!
//! 使用 `target_os = "none"` 区分裸机（内核）和宿主机（测试/clippy）：
//! - `cfg(target_os = "none")`: 裸机编译，执行真实的特权 asm
//! - `cfg(not(target_os = "none"))`: 宿主机编译，no-op 或 mock
//!
//! 不能使用 `cfg(test)` 因为 `test` 仅对当前正在测试的 crate 生效，
//! 不对其依赖传播。当 simplekernel 运行测试时，arch-traits 作为依赖
//! 被编译时 `cfg(test)` 为 false，会导致特权 asm 在用户态执行 → SIGILL。

// ─── core_id ─────────────────────────────────────────────────────────

/// 读取当前核心 ID。
///
/// - RISC-V: 从 `tp` 寄存器读取（boot.S 中设置为 hart ID）
/// - AArch64: 从 `MPIDR_EL1.Aff0` 读取
/// - 宿主机: 每线程分配唯一 ID（thread_local）
#[inline(always)]
pub fn core_id() -> usize {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    {
        let id: usize;
        // SAFETY: tp 寄存器在 boot.S 中设置为 hart ID
        unsafe { core::arch::asm!("mv {id}, tp", id = out(reg) id) };
        id
    }
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        let mpidr: u64;
        // SAFETY: MPIDR_EL1 在 EL1 下始终可读
        unsafe { core::arch::asm!("mrs {mpidr}, mpidr_el1", mpidr = out(reg) mpidr) };
        (mpidr & 0xFF) as usize
    }
    #[cfg(not(target_os = "none"))]
    {
        // 宿主机——为每个线程分配唯一 core_id（支持 SpinLock 多线程测试）
        use core::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        #[cfg(test)]
        {
            use std::cell::Cell;
            thread_local! {
                static TID: Cell<usize> =
                    Cell::new(COUNTER.fetch_add(1, Ordering::Relaxed));
            }
            TID.with(|id| id.get())
        }
        #[cfg(not(test))]
        {
            let _ = &COUNTER; // suppress unused warning
            0 // clippy/check 占位
        }
    }
}

// ─── 中断控制 ─────────────────────────────────────────────────────────

/// 查询当前中断是否启用。
#[inline(always)]
pub fn irq_enabled() -> bool {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    {
        riscv::register::sstatus::read().sie()
    }
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        let daif: u64;
        // SAFETY: DAIF 在 EL1 下可读
        unsafe { core::arch::asm!("mrs {daif}, daif", daif = out(reg) daif) };
        (daif & (1 << 7)) == 0
    }
    #[cfg(not(target_os = "none"))]
    {
        false // 宿主机——中断概念不适用
    }
}

/// 禁用中断。
#[inline(always)]
pub fn irq_disable() {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    riscv::interrupt::supervisor::disable();
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    // SAFETY: msr daifset 是 EL1 特权指令
    unsafe {
        core::arch::asm!("msr daifset, #2")
    };
    // 宿主机: no-op
}

/// 启用中断。
///
/// # Safety
/// 调用方必须确保在启用中断后不会违反临界区不变量。
#[inline(always)]
pub unsafe fn irq_enable() {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    // SAFETY: 由调用方保证安全性
    unsafe {
        riscv::interrupt::supervisor::enable()
    };
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    // SAFETY: msr daifclr 是 EL1 特权指令
    unsafe {
        core::arch::asm!("msr daifclr, #2")
    };
    // 宿主机: no-op
}

// ─── TLB ──────────────────────────────────────────────────────────────

/// 刷新 TLB。
#[inline(always)]
pub fn flush_tlb() {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    // SAFETY: sfence.vma 是 S-mode 特权指令
    unsafe {
        core::arch::asm!("sfence.vma")
    };
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    // SAFETY: tlbi/dsb/isb 是 EL1 特权指令
    unsafe {
        core::arch::asm!("tlbi vmalle1", "dsb sy", "isb")
    };
    // 宿主机: no-op
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_id_unique_per_thread() {
        use std::collections::HashSet;
        use std::sync::{Arc, Mutex};
        use std::thread;

        let ids = Arc::new(Mutex::new(HashSet::new()));
        let mut handles = Vec::new();

        for _ in 0..4 {
            let ids = Arc::clone(&ids);
            handles.push(thread::spawn(move || {
                let id = core_id();
                ids.lock().expect("mutex").insert(id);
            }));
        }
        for h in handles {
            h.join().expect("thread");
        }
        // 4 个线程应该得到 4 个不同的 ID
        assert_eq!(ids.lock().expect("mutex").len(), 4);
    }

    #[test]
    fn irq_disabled_on_host() {
        assert!(!irq_enabled());
    }

    #[test]
    fn irq_disable_is_noop_on_host() {
        irq_disable(); // 不应 panic 或 SIGILL
    }
}
