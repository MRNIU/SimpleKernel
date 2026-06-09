// Copyright The SimpleKernel Contributors

//! 启动期 DTB 生命周期管理。
//!
//! 本 crate 负责把 bootloader 传入的 DTB 从外部 blob 复制到内核自有、
//! 页对齐的固定 storage。后续 FDT 解析应只使用复制后的 bytes。

#![cfg_attr(not(test), no_std)]
#![feature(sync_unsafe_cell)]

use core::cell::SyncUnsafeCell;
use core::fmt;

/// 第一版内核自有 DTB storage 上限。
pub const MAX_DTB_SIZE: usize = 256 * 1024;

const FDT_MAGIC: u32 = 0xd00d_feed;
const FDT_HEADER_SIZE: usize = 40;

const _: [(); 0] = [(); MAX_DTB_SIZE % config::PAGE_SIZE];

#[repr(C, align(4096))]
struct DtbStorage([u8; MAX_DTB_SIZE]);

/// 已复制到内核自有 storage 的 DTB。
#[derive(Debug)]
pub struct BootFdt {
    raw_addr: usize,
    storage_addr: usize,
    bytes: &'static [u8],
}

impl BootFdt {
    /// 原始 bootloader DTB 地址，仅用于诊断。
    pub const fn raw_addr(&self) -> usize {
        self.raw_addr
    }

    /// 内核自有 DTB 副本起始地址。
    pub const fn storage_addr(&self) -> usize {
        self.storage_addr
    }

    /// DTB 副本 bytes。
    pub const fn bytes(&self) -> &'static [u8] {
        self.bytes
    }

    /// DTB `totalsize`。
    pub const fn total_size(&self) -> usize {
        self.bytes.len()
    }
}

/// 内核自有 DTB storage 的页对齐范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageRegion {
    /// storage 起始地址。
    pub start: usize,
    /// storage 保留字节数。
    pub len: usize,
}

impl StorageRegion {
    /// storage 覆盖页数。
    pub const fn page_count(self) -> usize {
        self.len / config::PAGE_SIZE
    }
}

/// DTB 初始化错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootFdtError {
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
}

impl fmt::Display for BootFdtError {
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
        }
    }
}

impl core::error::Error for BootFdtError {}

static DTB_STORAGE: SyncUnsafeCell<DtbStorage> = SyncUnsafeCell::new(DtbStorage([0; MAX_DTB_SIZE]));
static BOOT_FDT: spin::Once<BootFdt> = spin::Once::new();

/// 从 bootloader 传入的原始 DTB 地址初始化 kernel-owned DTB 副本。
///
/// # Safety
///
/// `raw_addr` 必须指向 bootloader 提供的有效 DTB，且至少在本函数完成复制前保持可读。
pub unsafe fn init_from_raw(raw_addr: usize) -> Result<&'static BootFdt, BootFdtError> {
    if let Some(existing) = BOOT_FDT.get() {
        return Err(BootFdtError::AlreadyInitialized {
            existing_addr: existing.storage_addr(),
        });
    }

    let total_size = unsafe { validate_raw_header(raw_addr)? };
    let storage_ptr = DTB_STORAGE.get().cast::<u8>();

    // SAFETY: 调用方保证 raw_addr 指向 total_size 字节可读 DTB；storage_ptr 指向
    // 内核自有固定 storage，且 BOOT_FDT 尚未初始化，启动期单核路径没有并发写入者。
    unsafe {
        core::ptr::copy_nonoverlapping(raw_addr as *const u8, storage_ptr, total_size);
    }

    // SAFETY: 刚复制 total_size 字节到内核自有 static storage；该 storage 在内核生命周期内有效。
    let bytes = unsafe { core::slice::from_raw_parts(storage_ptr.cast_const(), total_size) };
    let boot_fdt = BootFdt {
        raw_addr,
        storage_addr: storage_ptr.addr(),
        bytes,
    };

    Ok(BOOT_FDT.call_once(|| boot_fdt))
}

/// 返回已初始化的 kernel-owned DTB。
pub fn get() -> Option<&'static BootFdt> {
    BOOT_FDT.get()
}

/// 返回 DTB 固定 storage 范围。只有初始化后才需要修改映射权限。
pub fn storage_region() -> Option<StorageRegion> {
    BOOT_FDT.get().map(|boot_fdt| StorageRegion {
        start: boot_fdt.storage_addr(),
        len: MAX_DTB_SIZE,
    })
}

unsafe fn validate_raw_header(raw_addr: usize) -> Result<usize, BootFdtError> {
    if raw_addr == 0 {
        return Err(BootFdtError::NullRawAddr);
    }

    // SAFETY: 调用方保证 raw_addr 指向至少 FDT header 可读的 DTB。
    let magic = unsafe { read_be_u32(raw_addr as *const u8) };
    if magic != FDT_MAGIC {
        return Err(BootFdtError::InvalidMagic { raw_addr, magic });
    }

    // SAFETY: 调用方保证 raw_addr 指向至少 FDT header 可读的 DTB；totalsize 位于 header + 4。
    let total_size = unsafe { read_be_u32((raw_addr as *const u8).add(4)) as usize };
    validate_total_size(raw_addr, total_size)?;
    Ok(total_size)
}

fn validate_total_size(raw_addr: usize, total_size: usize) -> Result<(), BootFdtError> {
    if total_size < FDT_HEADER_SIZE {
        return Err(BootFdtError::HeaderTooSmall {
            raw_addr,
            total_size,
        });
    }

    if total_size > MAX_DTB_SIZE {
        return Err(BootFdtError::TooLarge {
            raw_addr,
            total_size,
            max_size: MAX_DTB_SIZE,
        });
    }

    Ok(())
}

unsafe fn read_be_u32(ptr: *const u8) -> u32 {
    let mut bytes = [0u8; 4];
    // SAFETY: 调用方保证 ptr 指向至少 4 字节可读内存；copy_nonoverlapping 不要求源对齐。
    unsafe {
        core::ptr::copy_nonoverlapping(ptr, bytes.as_mut_ptr(), bytes.len());
    }
    u32::from_be_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW_ADDR: usize = 0x8020_0000;

    #[test]
    fn validate_total_size_accepts_header_sized_dtb() {
        assert_eq!(validate_total_size(RAW_ADDR, FDT_HEADER_SIZE), Ok(()));
    }

    #[test]
    fn validate_total_size_rejects_too_small_header() {
        assert_eq!(
            validate_total_size(RAW_ADDR, FDT_HEADER_SIZE - 1),
            Err(BootFdtError::HeaderTooSmall {
                raw_addr: RAW_ADDR,
                total_size: FDT_HEADER_SIZE - 1,
            })
        );
    }

    #[test]
    fn validate_total_size_rejects_oversized_dtb() {
        assert_eq!(
            validate_total_size(RAW_ADDR, MAX_DTB_SIZE + 1),
            Err(BootFdtError::TooLarge {
                raw_addr: RAW_ADDR,
                total_size: MAX_DTB_SIZE + 1,
                max_size: MAX_DTB_SIZE,
            })
        );
    }

    #[test]
    fn read_be_u32_accepts_unaligned_input() {
        let bytes = [0, 0xd0, 0x0d, 0xfe, 0xed];
        let ptr = bytes[1..].as_ptr();
        // SAFETY: ptr 指向 bytes[1..] 的 4 字节有效内存。
        let value = unsafe { read_be_u32(ptr) };
        assert_eq!(value, FDT_MAGIC);
    }
}
