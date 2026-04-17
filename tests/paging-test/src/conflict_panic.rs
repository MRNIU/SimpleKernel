//! should_panic 测试——identity_map_range 权限冲突（flags 不同）时应 panic。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::{PhysAddr, VirtAddr};
use paging::{PageTable, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test, should_panic);

fn run_test() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let conflict_va = VirtAddr::new(0x10_2000);
    let conflict_pa = PhysAddr::new(0x10_2000);
    // 先以 kernel_ro 占位
    pt.create_pte(conflict_va, conflict_pa, PteFlags::kernel_ro())
        .expect("占位应成功");

    // 以 kernel_rw 对同一区域 identity_map_range——flags 冲突，应 panic
    pt.identity_map_range(
        PhysAddr::new(0x10_0000),
        PhysAddr::new(0x10_3000),
        PteFlags::kernel_rw(),
    );
}
