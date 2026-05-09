// Copyright The SimpleKernel Contributors

//! TLB shootdown 系统测试——验证在线从核参与页表权限更新同步。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

use core::hint::spin_loop;

use frame_allocator::AllocatedFrames;
use memory_types::VirtAddr;
use paging::{PteFlags, PteFlagsOps};
use simplekernel::{CORE_COUNT, boot::InitLevel, tlb_shootdown};

test_harness::test_main!(InitLevel::Full, run_tests);

const ONLINE_WAIT_ROUNDS: usize = 1_000_000;

fn run_tests() {
    wait_all_cores_online();
    test_update_range_flags_flushes_online_cores();
    log::info!("test update_range_flags_flushes_online_cores ... ok");
    log::info!("tlb-shootdown-test: all 1 tests passed");
}

/// 等待 FDT 中发现的所有 CPU 完成 SMP 上线流程。
fn wait_all_cores_online() {
    let expected = *CORE_COUNT
        .get()
        .expect("tlb shootdown 测试读取 CORE_COUNT 失败");

    for _ in 0..ONLINE_WAIT_ROUNDS {
        if tlb_shootdown::online_core_count() == expected {
            return;
        }
        spin_loop();
    }

    panic!(
        "tlb shootdown 测试等待 CPU 上线超时: expected={}, actual={}",
        expected,
        tlb_shootdown::online_core_count()
    );
}

/// 批量权限更新应在所有在线 CPU 上完成 TLB shootdown。
fn test_update_range_flags_flushes_online_cores() {
    let frames = AllocatedFrames::alloc(2).expect("tlb shootdown 测试分配 2 页物理帧失败");
    let vaddr = frames.start_paddr().to_virt();
    let page_table = paging::kernel_page_table();

    page_table.update_range_flags(vaddr, frames.page_count(), PteFlags::kernel_ro());
    assert_page_flags(vaddr, frames.page_count(), PteFlags::kernel_ro());

    page_table.update_range_flags(vaddr, frames.page_count(), PteFlags::kernel_rw());
    assert_page_flags(vaddr, frames.page_count(), PteFlags::kernel_rw());
}

/// 校验页表项权限已经更新到预期值。
fn assert_page_flags(vaddr: VirtAddr, count: usize, expected: PteFlags) {
    let page_table = paging::kernel_page_table();

    for index in 0..count {
        let addr = vaddr + index * config::PAGE_SIZE;
        let (_, flags) = page_table
            .get_mapping(addr)
            .expect("tlb shootdown 测试查询页表项失败");
        assert!(
            flags.contains(expected),
            "tlb shootdown 测试页权限不匹配: index={}, addr={:#x}, flags={:?}, expected={:?}",
            index,
            addr.as_usize(),
            flags,
            expected
        );
    }
}
