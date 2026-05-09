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

    for core_id in 0..config::MAX_CORE_COUNT {
        if target_mask & (1usize << core_id) != 0 {
            Arch::send_ipi(core_id);
        }
    }

    for (core_id, ack) in ACK_GENERATION.iter().enumerate() {
        if target_mask & (1usize << core_id) == 0 {
            continue;
        }
        while ack.load(Ordering::Acquire) != generation {
            core::hint::spin_loop();
        }
    }

    BROADCAST_LOCK.store(false, Ordering::Release);
}
