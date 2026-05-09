// Copyright The SimpleKernel Contributors

//! 跨核 TLB shootdown 协调。
//!
//! `tlb` crate 只知道“需要广播”的抽象回调；本模块负责把请求编码为
//! IPI payload、发送到其他在线核心，并等待目标核心完成本核 TLB flush。

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use tlb::{self, TlbFlushRequest};

use crate::arch::{Arch, ArchOps};

const REQUEST_NONE: usize = 0;
const REQUEST_ALL: usize = 1;
const REQUEST_PAGE: usize = 2;
const ONLINE_WAIT_ROUNDS: usize = 1_000_000;
const ACK_WAIT_ROUNDS: usize = 10_000_000;

static ONLINE_CORES: AtomicUsize = AtomicUsize::new(0);
static BROADCAST_LOCK: AtomicBool = AtomicBool::new(false);
static REQUEST_KIND: AtomicUsize = AtomicUsize::new(REQUEST_NONE);
static REQUEST_ADDR: AtomicUsize = AtomicUsize::new(0);
static REQUEST_GENERATION: AtomicUsize = AtomicUsize::new(0);
static ACK_GENERATION: [AtomicUsize; config::MAX_CORE_COUNT] =
    [const { AtomicUsize::new(0) }; config::MAX_CORE_COUNT];

/// 注册 shootdown 回调，并标记当前核心在线。
///
/// 必须在主核中断控制器初始化完成后调用；此时本核可以发送 IPI。
pub fn init_primary() {
    tlb::register_tlb_shootdown(broadcast);
    mark_current_core_online();
}

/// 标记当前核心已能接收 IPI。
///
/// 从核必须在本核中断控制器初始化并启用 IRQ 后调用。
pub fn mark_current_core_online() {
    let core_id = per_cpu::current_core_id();
    assert!(
        core_id < config::MAX_CORE_COUNT,
        "tlb_shootdown: core_id {} 超出 MAX_CORE_COUNT {}",
        core_id,
        config::MAX_CORE_COUNT
    );
    ONLINE_CORES.fetch_or(1usize << core_id, Ordering::Release);
}

/// 返回当前已标记可接收 TLB shootdown IPI 的 CPU 数量。
pub fn online_core_count() -> usize {
    ONLINE_CORES.load(Ordering::Acquire).count_ones() as usize
}

/// 返回当前已标记可接收 TLB shootdown IPI 的 CPU 位图。
pub fn online_core_mask() -> usize {
    ONLINE_CORES.load(Ordering::Acquire)
}

/// 当前所有 FDT 发现的 CPU 是否都已完成 SMP online。
pub fn all_discovered_cores_online() -> bool {
    let discovered = crate::cpu_topology::topology().discovered_core_count();
    let expected = expected_online_mask(discovered);
    online_core_mask() & expected == expected
}

/// 等待所有 FDT 发现的 CPU 完成 SMP online。
///
/// # Panics
/// 当从核未在有限自旋内上线时 panic，并打印缺失 CPU 位图。
pub fn wait_for_all_discovered_cores_online() {
    let discovered = crate::cpu_topology::topology().discovered_core_count();
    let expected = expected_online_mask(discovered);

    for _ in 0..ONLINE_WAIT_ROUNDS {
        if online_core_mask() & expected == expected {
            return;
        }
        core::hint::spin_loop();
    }

    let actual = online_core_mask();
    panic!(
        "SMP: 等待所有 CPU online 超时: discovered={}, expected_mask={:#x}, actual_mask={:#x}, missing_mask={:#x}",
        discovered,
        expected,
        actual,
        expected & !actual
    );
}

/// 处理当前核心收到的 TLB shootdown IPI。
pub fn handle_ipi() {
    let core_id = per_cpu::current_core_id();
    assert!(
        core_id < config::MAX_CORE_COUNT,
        "tlb_shootdown: IPI 处理 core_id {} 超出 MAX_CORE_COUNT {}",
        core_id,
        config::MAX_CORE_COUNT
    );

    let generation = REQUEST_GENERATION.load(Ordering::Acquire);
    if generation == 0 {
        return;
    }

    match REQUEST_KIND.load(Ordering::Relaxed) {
        REQUEST_ALL => tlb::flush_tlb_local(),
        REQUEST_PAGE => tlb::flush_tlb_page_local(REQUEST_ADDR.load(Ordering::Relaxed)),
        REQUEST_NONE => return,
        kind => panic!("tlb_shootdown: 未知请求类型 {kind}"),
    }

    ACK_GENERATION[core_id].store(generation, Ordering::Release);
}

/// 触发 TLB shootdown ack 超时路径，供独立系统测试验证诊断 fail-fast。
///
/// # Panics
/// 总是触发 ack timeout panic。
#[cfg(feature = "test-support")]
pub fn trigger_ack_timeout_for_test() {
    let mut generation = REQUEST_GENERATION.load(Ordering::Acquire).wrapping_add(1);
    if generation == 0 {
        generation = 1;
    }
    wait_for_acks_or_panic(
        1usize << (config::MAX_CORE_COUNT - 1),
        generation,
        REQUEST_ALL,
        0,
        per_cpu::current_core_id(),
        8,
    );
}

fn broadcast(request: TlbFlushRequest) {
    assert!(
        !interrupt_state::is_in_interrupt(),
        "tlb_shootdown: 不支持在中断上下文中发起广播"
    );

    let self_id = per_cpu::current_core_id();
    assert!(
        self_id < config::MAX_CORE_COUNT,
        "tlb_shootdown: 发起广播 core_id {} 超出 MAX_CORE_COUNT {}",
        self_id,
        config::MAX_CORE_COUNT
    );
    let self_bit = 1usize << self_id;
    let target_mask = ONLINE_CORES.load(Ordering::Acquire) & !self_bit;
    if target_mask == 0 {
        return;
    }

    assert!(
        interrupt_state::is_enabled(),
        "tlb_shootdown: 发起跨核广播前必须处于 IRQ enabled 状态，避免等待远端 ack 时形成不可恢复等待"
    );

    while BROADCAST_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }

    let _irq = interrupt_state::HeldInterrupts::hold();
    let (kind, addr) = match request {
        TlbFlushRequest::All => (REQUEST_ALL, 0),
        TlbFlushRequest::Page(vaddr) => (REQUEST_PAGE, vaddr),
    };

    REQUEST_ADDR.store(addr, Ordering::Relaxed);
    REQUEST_KIND.store(kind, Ordering::Relaxed);
    let generation = REQUEST_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    core::sync::atomic::fence(Ordering::Release);

    for core_id in 0..config::MAX_CORE_COUNT {
        if target_mask & (1usize << core_id) != 0 {
            Arch::send_ipi(core_id);
        }
    }

    wait_for_acks_or_panic(
        target_mask,
        generation,
        kind,
        addr,
        self_id,
        ACK_WAIT_ROUNDS,
    );

    BROADCAST_LOCK.store(false, Ordering::Release);
}

fn wait_for_acks_or_panic(
    target_mask: usize,
    generation: usize,
    kind: usize,
    addr: usize,
    initiator_core_id: usize,
    max_rounds: usize,
) {
    for _ in 0..max_rounds {
        if missing_ack_mask(target_mask, generation) == 0 {
            return;
        }
        core::hint::spin_loop();
    }

    let missing = missing_ack_mask(target_mask, generation);
    panic!(
        "tlb_shootdown: 等待远端 ack 超时: initiator_core={}, target_mask={:#x}, missing_ack_mask={:#x}, generation={}, request_kind={}, request_addr={:#x}, max_rounds={}",
        initiator_core_id,
        target_mask,
        missing,
        generation,
        request_kind_name(kind),
        addr,
        max_rounds
    );
}

fn missing_ack_mask(target_mask: usize, generation: usize) -> usize {
    let mut missing = 0;
    for (core_id, ack) in ACK_GENERATION.iter().enumerate() {
        let core_bit = 1usize << core_id;
        if target_mask & core_bit != 0 && ack.load(Ordering::Acquire) != generation {
            missing |= core_bit;
        }
    }
    missing
}

fn request_kind_name(kind: usize) -> &'static str {
    match kind {
        REQUEST_ALL => "all",
        REQUEST_PAGE => "page",
        REQUEST_NONE => "none",
        _ => "unknown",
    }
}

fn expected_online_mask(core_count: usize) -> usize {
    assert!(
        (1..=config::MAX_CORE_COUNT).contains(&core_count),
        "SMP: discovered core count {} 不在 1..={} 内",
        core_count,
        config::MAX_CORE_COUNT
    );
    (1usize << core_count) - 1
}
