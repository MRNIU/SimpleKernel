// Copyright The SimpleKernel Contributors

use core::fmt;

const FDT_HEADER_SIZE: usize = 40;

/// FDT 初始化或查询错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdtError {
    /// 原始 DTB 地址为空。
    NullRawAddr,
    /// FDT magic 不匹配。
    InvalidMagic { raw_addr: usize, magic: u32 },
    /// `totalsize` 小于 FDT header。
    HeaderTooSmall { raw_addr: usize, total_size: usize },
    /// `totalsize` 超过内核固定 storage 上限。
    TooLarge {
        raw_addr: usize,
        total_size: usize,
        max_size: usize,
    },
    /// 全局 DTB storage 已初始化。
    AlreadyInitialized { existing_addr: usize },
    /// FDT header 解析失败。
    InvalidHeader,
    /// 找不到所需节点。
    NodeNotFound,
    /// 找不到所需属性。
    PropertyNotFound,
    /// FDT 解析失败。
    ParseFailed,
    /// 属性大小不匹配。
    InvalidPropertySize,
    /// 当前不支持该硬件布局。
    UnsupportedLayout,
}

impl fmt::Display for FdtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::NullRawAddr => write!(f, "DTB 原始地址为空"),
            Self::InvalidMagic { raw_addr, magic } => {
                write!(
                    f,
                    "DTB magic 无效: raw_addr={raw_addr:#x}, magic={magic:#x}"
                )
            }
            Self::HeaderTooSmall {
                raw_addr,
                total_size,
            } => write!(
                f,
                "DTB totalsize 小于 header: raw_addr={raw_addr:#x}, totalsize={total_size}, header={FDT_HEADER_SIZE}"
            ),
            Self::TooLarge {
                raw_addr,
                total_size,
                max_size,
            } => write!(
                f,
                "DTB 超出内核 storage 上限: raw_addr={raw_addr:#x}, totalsize={total_size}, max_size={max_size}"
            ),
            Self::AlreadyInitialized { existing_addr } => {
                write!(f, "DTB storage 已初始化: existing_addr={existing_addr:#x}")
            }
            _ => fmt::Debug::fmt(self, f),
        }
    }
}

impl core::error::Error for FdtError {}
