// Copyright The SimpleKernel Contributors

//! 上层能力接口。

/// 块设备操作结果。
///
/// # Errors
///
/// 块设备调用在缓冲区长度非法、扇区越界或底层 I/O 失败时返回 [`BlockError`]。
pub type BlockResult<T> = Result<T, BlockError>;

/// 块设备错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// 调用方传入的缓冲区长度不是一个完整扇区。
    InvalidBuffer {
        /// 实际缓冲区长度。
        len: usize,
        /// 设备要求的扇区大小。
        sector_size: usize,
    },
    /// 调用方访问了设备容量之外的扇区。
    SectorOutOfRange {
        /// 请求的扇区号。
        sector: u64,
        /// 设备总扇区数。
        sector_count: u64,
    },
    /// 底层设备 I/O 失败。
    Io,
}

/// 本地块设备能力接口。
pub trait BlockDevice: Send + Sync {
    /// 单个扇区大小，单位为字节。
    fn sector_size(&self) -> usize;

    /// 设备总扇区数。
    fn sector_count(&self) -> u64;

    /// 设备容量，单位为字节。
    fn capacity(&self) -> u64 {
        self.sector_count()
            .saturating_mul(self.sector_size() as u64)
    }

    /// 读取一个完整扇区。
    ///
    /// # Errors
    ///
    /// 缓冲区长度不等于扇区大小、扇区号越界或底层设备 I/O 失败时返回 [`BlockError`]。
    fn read_sector(&self, sector: u64, buf: &mut [u8]) -> BlockResult<()>;

    /// 写入一个完整扇区。
    ///
    /// # Errors
    ///
    /// 缓冲区长度不等于扇区大小、扇区号越界或底层设备 I/O 失败时返回 [`BlockError`]。
    fn write_sector(&self, sector: u64, buf: &[u8]) -> BlockResult<()>;
}

/// 校验整扇区 I/O 的缓冲区长度和扇区边界。
///
/// # Errors
///
/// 缓冲区长度不是完整扇区，或扇区号不在设备容量范围内时返回 [`BlockError`]。
pub fn validate_sector_io(
    sector: u64,
    buf_len: usize,
    sector_size: usize,
    sector_count: u64,
) -> BlockResult<()> {
    if buf_len != sector_size {
        return Err(BlockError::InvalidBuffer {
            len: buf_len,
            sector_size,
        });
    }

    if sector >= sector_count {
        return Err(BlockError::SectorOutOfRange {
            sector,
            sector_count,
        });
    }

    Ok(())
}

/// 设备能力分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityType {
    /// 块设备能力。
    Block,
}

/// 设备实例暴露给上层的 typed capability。
#[derive(Clone, Copy)]
pub enum DeviceCapability {
    /// 块设备能力。
    Block(&'static dyn BlockDevice),
}

impl DeviceCapability {
    /// 返回 capability 分类。
    pub const fn capability_type(self) -> CapabilityType {
        match self {
            Self::Block(_) => CapabilityType::Block,
        }
    }
}
