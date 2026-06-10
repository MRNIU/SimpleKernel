// Copyright The SimpleKernel Contributors

//! 设备管理框架——设备注册、查找、VirtIO 子系统。
//!
//! 本模块通过 FDT 枚举发现设备，调用对应驱动探测函数，
//! 并将成功探测的设备注册为 typed capability。
//!
//! 架构：
//! - `hal.rs`：`virtio-drivers` crate 的 HAL 实现
//! - `block.rs`：本地 `BlockDevice` 能力门面
//! - `manager.rs`：迁移期内部设备记录
//! - `platform_bus.rs`：FDT 遍历 → 驱动匹配
//! - `virtio.rs`：VirtIO 设备探测与管理

pub mod block;
mod error;
pub mod hal;
pub(crate) mod manager;
pub mod platform_bus;
pub mod virtio;

pub use error::DeviceError;

/// 设备类型分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// 块设备（磁盘/分区）
    Block,
    /// 控制台/串口
    Console,
    /// 网络接口
    Net,
    /// 其他设备
    Other,
}

/// 设备 trait——所有设备驱动实现此接口。
pub trait Device: Send + Sync {
    /// 设备名称（如 "virtio-blk0"）。
    fn name(&self) -> &str;

    /// 设备类型。
    fn device_type(&self) -> DeviceType;
}

/// 返回 registry 当前记录的设备实例数量。
#[must_use]
pub fn device_count() -> usize {
    block::registered_device_count()
}

/// 初始化设备子系统——扫描 FDT 并探测所有设备。
///
/// 在页表激活和中断初始化之后调用。
///
/// # Panics
/// Full 初始化要求 FDT 平台输入已就绪且可解析；若 platform bus 发现 FDT 缺失、
/// 解析失败或已匹配设备节点的 `reg` 属性非法，会立即 panic。
pub fn device_init() {
    log::info!("DeviceInit: scanning FDT...");
    manager::init();
    platform_bus::probe_all();
    let default_block = block::default_block_device_id().unwrap_or_else(|| {
        panic!("DeviceInit: Full 初始化完成后缺少默认 Block capability，无法继续初始化文件系统")
    });
    let count = device_count();
    log::info!(
        "DeviceInit complete: {} devices enumerated, default_block_device={}",
        count,
        default_block.raw()
    );
}
