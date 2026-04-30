//! should_panic 测试——构造 PTE 时物理地址必须页对齐。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::PhysAddr;
use page_table_entry::{PageTableEntry, PteFlags, PteFlagsOps, PteOps};

test_harness::test_main!(
    simplekernel::boot::InitLevel::Memory,
    run_test,
    should_panic
);

/// 未对齐物理地址构造 PTE 应 panic，避免 PPN 编码静默截断低位。
fn run_test() {
    let pa = PhysAddr::new(0x8020_0123);
    let _ = PageTableEntry::new(pa, PteFlags::kernel_rw());
}
