//! 设备管理器——全局设备注册与查找。

use alloc::boxed::Box;
use alloc::vec::Vec;

use sync::SpinLock;

use super::{Device, DeviceType};

/// 全局设备管理器。
static DEVICE_MANAGER: SpinLock<Vec<Box<dyn Device>>> = SpinLock::new(Vec::new(), "dev_mgr");

/// 初始化设备管理器。
pub fn init() {
    // Vec 已在 static 中初始化，此处仅作标记
    log::debug!("DeviceManager: initialized");
}

/// 注册一个已探测成功的设备。
pub fn register_device(device: Box<dyn Device>) {
    let name = alloc::format!("{}", device.name());
    let dtype = device.device_type();
    DEVICE_MANAGER.lock().push(device);
    log::info!("DeviceManager: registered {:?} \"{}\"", dtype, name);
}

/// 返回已注册设备总数。
pub fn device_count() -> usize {
    DEVICE_MANAGER.lock().len()
}

/// 按设备类型查找第一个匹配的设备索引。
#[expect(dead_code, reason = "公开 API，供驱动层和文件系统层后续使用")]
pub fn find_by_type(dtype: DeviceType) -> Option<usize> {
    DEVICE_MANAGER
        .lock()
        .iter()
        .position(|d| d.device_type() == dtype)
}

/// 对指定索引的设备执行操作。
///
/// 通过闭包访问设备引用，避免持有锁的生命周期泄漏。
#[expect(dead_code, reason = "公开 API，供驱动层和文件系统层后续使用")]
pub fn with_device<F, R>(index: usize, f: F) -> Option<R>
where
    F: FnOnce(&dyn Device) -> R,
{
    let devices = DEVICE_MANAGER.lock();
    devices.get(index).map(|d| f(d.as_ref()))
}
