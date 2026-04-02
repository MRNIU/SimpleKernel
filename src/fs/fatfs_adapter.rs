//! VirtIO 块设备 → 字节级 I/O 适配器。
//!
//! 将 VirtIO 块设备的扇区粒度 I/O 转换为 `fatfs` crate 所需的
//! 字节级 `Read`/`Write`/`Seek` 接口。
//!
//! 核心挑战：VirtIO 块设备以 512 字节扇区为单位操作，
//! 而文件系统需要任意偏移的字节级读写。
//! 解决方案：扇区对齐的 read-modify-write。
//!
//! 当前为基础实现——供未来 `fatfs` crate 集成使用。

use super::vfs::FsError;

/// 块设备扇区大小（字节）。
pub const SECTOR_SIZE: usize = 512;

/// VirtIO 块设备的字节级 I/O 适配器。
///
/// 维护当前读写位置（`position`），将字节级操作转换为扇区级操作。
///
/// # 用法
///
/// ```ignore
/// let mut adapter = VirtioBlockAdapter::new();
/// adapter.seek_to(1024);
/// let n = adapter.read_bytes(&mut buf)?;
/// ```
pub struct VirtioBlockAdapter {
    /// 当前字节偏移量
    position: u64,
}

impl VirtioBlockAdapter {
    /// 创建新的适配器，位置初始化为 0。
    pub fn new() -> Self {
        Self { position: 0 }
    }

    /// 设置读写位置。
    pub fn seek_to(&mut self, pos: u64) {
        self.position = pos;
    }

    /// 获取当前位置。
    pub fn position(&self) -> u64 {
        self.position
    }

    /// 从当前位置读取数据到 `buf`，返回实际读取字节数。
    ///
    /// 内部将字节级读取拆分为一个或多个扇区读取。
    ///
    /// # Errors
    ///
    /// 块设备不可用或 I/O 失败时返回 `IoError`。
    pub fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        let blk = crate::device::virtio::virtio_blk().ok_or(FsError::IoError)?;
        let mut blk = blk.lock();

        let mut bytes_read = 0;
        let mut remaining = buf;

        while !remaining.is_empty() {
            let sector = self.position / SECTOR_SIZE as u64;
            let offset_in_sector = (self.position % SECTOR_SIZE as u64) as usize;

            let mut sector_buf = [0u8; SECTOR_SIZE];
            blk.read_blocks(sector as usize, &mut sector_buf)
                .map_err(|_| FsError::IoError)?;

            let available = SECTOR_SIZE - offset_in_sector;
            let n = remaining.len().min(available);
            remaining[..n].copy_from_slice(&sector_buf[offset_in_sector..offset_in_sector + n]);

            remaining = &mut remaining[n..];
            self.position += n as u64;
            bytes_read += n;
        }

        Ok(bytes_read)
    }

    /// 从当前位置写入 `data`，返回实际写入字节数。
    ///
    /// 使用 read-modify-write 处理非扇区对齐的写入。
    ///
    /// # Errors
    ///
    /// 块设备不可用或 I/O 失败时返回 `IoError`。
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<usize, FsError> {
        let blk = crate::device::virtio::virtio_blk().ok_or(FsError::IoError)?;
        let mut blk = blk.lock();

        let mut bytes_written = 0;
        let mut remaining = data;

        while !remaining.is_empty() {
            let sector = self.position / SECTOR_SIZE as u64;
            let offset_in_sector = (self.position % SECTOR_SIZE as u64) as usize;

            let mut sector_buf = [0u8; SECTOR_SIZE];

            // Read-modify-write：先读取当前扇区
            if offset_in_sector != 0 || remaining.len() < SECTOR_SIZE {
                blk.read_blocks(sector as usize, &mut sector_buf)
                    .map_err(|_| FsError::IoError)?;
            }

            // 写入数据到扇区缓冲
            let available = SECTOR_SIZE - offset_in_sector;
            let n = remaining.len().min(available);
            sector_buf[offset_in_sector..offset_in_sector + n].copy_from_slice(&remaining[..n]);

            // 写回扇区
            blk.write_blocks(sector as usize, &sector_buf)
                .map_err(|_| FsError::IoError)?;

            remaining = &remaining[n..];
            self.position += n as u64;
            bytes_written += n;
        }

        Ok(bytes_written)
    }
}

impl Default for VirtioBlockAdapter {
    fn default() -> Self {
        Self::new()
    }
}
