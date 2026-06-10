// Copyright The SimpleKernel Contributors

//! VirtIO 设备探测与管理——利用 `virtio-drivers` crate。
//!
//! 通过 MMIO transport 探测 VirtIO 设备类型，对支持的设备（当前仅块设备）
//! 执行完整初始化并注册为块设备 capability。
//!
//! [VirtIO spec §4.2 Virtio Over MMIO](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)

mod block_device;
mod mmio;

pub use block_device::{VirtIOBlockDevice, VirtIOBlockLock, VirtIOBlockRecord};
pub(super) use mmio::{FDT_COMPATIBLES_MMIO, probe_mmio_descriptor};
pub use mmio::{VirtioMmioProbeResult, probe_mmio_block, probe_mmio_device};
