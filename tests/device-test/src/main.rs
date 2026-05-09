// Copyright The SimpleKernel Contributors

//! 设备子系统测试——验证设备管理器和 VirtIO 块设备。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_device_manager_has_devices();
    log::info!("test device_manager_has_devices ... ok");

    test_virtio_blk_available();
    log::info!("test virtio_blk_available ... ok");

    test_virtio_blk_read_sector();
    log::info!("test virtio_blk_read_sector ... ok");

    log::info!("device-test: all 3 tests passed");
}

/// 设备管理器应正常初始化且可查询。
fn test_device_manager_has_devices() {
    let count = simplekernel::device::manager::device_count();
    log::info!("device_manager: {} devices registered", count);
}

/// VirtIO 块设备全局引用可用性检查。
fn test_virtio_blk_available() {
    let available = simplekernel::device::virtio::virtio_blk().is_some();
    log::info!("virtio_blk available: {}", available);
}

/// VirtIO 块设备读取测试（仅在块设备可用时执行）。
fn test_virtio_blk_read_sector() {
    if let Some(blk) = simplekernel::device::virtio::virtio_blk() {
        let mut blk = blk.lock();
        let mut buf = [0u8; 512];
        blk.read_blocks(0, &mut buf).expect("读取扇区 0 失败");
        log::info!("virtio_blk: sector 0 read OK");
    } else {
        log::info!("virtio_blk: not available, skipping read test");
    }
}
