//! should_panic 测试——identity_map_range 对无效范围（start == end）应 panic。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::PhysAddr;
use paging::{PageTable, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test, should_panic);

fn run_test() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    pt.identity_map_range(
        PhysAddr::new(0x10_0000),
        PhysAddr::new(0x10_0000),
        PteFlags::kernel_rw(),
    );
}
