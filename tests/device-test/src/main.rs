// Copyright The SimpleKernel Contributors

//! 设备子系统测试——验证设备 registry 和块设备能力门面。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_device_registry_has_devices();
    log::info!("test device_registry_has_devices ... ok");

    test_block_device_facade_available();
    log::info!("test block_device_facade_available ... ok");

    test_block_device_read_sector();
    log::info!("test block_device_read_sector ... ok");

    log::info!("device-test: all 3 tests passed");
}

/// 设备 registry 应正常初始化且可查询。
fn test_device_registry_has_devices() {
    let count = simplekernel::device::device_count();
    assert!(
        count > 0,
        "device-test: Full 初始化后应至少注册一个设备，实际为 {}",
        count
    );
    assert!(
        simplekernel::device::block::default_block_device_id().is_some(),
        "device-test: Full 初始化后应注册默认 Block capability"
    );
}

/// 本地 BlockDevice 门面应可用并暴露容量信息。
fn test_block_device_facade_available() {
    let blk = simplekernel::device::block::block_device()
        .expect("device-test: Full 初始化后应注册本地 BlockDevice 门面");
    assert_eq!(
        blk.sector_size(),
        512,
        "device-test: QEMU VirtIO block 当前应暴露 512 字节扇区"
    );
    assert!(
        blk.sector_count() > 0,
        "device-test: BlockDevice 扇区数应大于 0，实际为 {}",
        blk.sector_count()
    );
    assert!(
        blk.capacity() >= blk.sector_size() as u64,
        "device-test: BlockDevice 容量应至少包含一个扇区，实际为 {}",
        blk.capacity()
    );
}

/// 本地 BlockDevice 门面读取测试。
fn test_block_device_read_sector() {
    let blk = simplekernel::device::block::block_device()
        .expect("device-test: Full 初始化后应注册本地 BlockDevice 门面");
    let mut buf = [0u8; 512];
    blk.read_sector(0, &mut buf)
        .expect("device-test: BlockDevice 门面读取 sector 0 应成功");
}
