//! should_panic 测试——identity_map_range 权限冲突（flags 不同）时应 panic。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::PhysAddr;
use paging::{PageTable, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test, should_panic);

fn run_test() {
    let pt = PageTable::create();

    // 先以 kernel_ro 建立一页映射
    pt.identity_map_range(
        PhysAddr::new(0x10_2000),
        PhysAddr::new(0x10_3000),
        PteFlags::kernel_ro(),
    );

    // 以 kernel_rw 对重叠区域再次 identity_map_range——flags 冲突，应 panic
    pt.identity_map_range(
        PhysAddr::new(0x10_0000),
        PhysAddr::new(0x10_3000),
        PteFlags::kernel_rw(),
    );
}
