//! VirtIO 块设备 → `fatfs` crate I/O 适配器。
//!
//! 将 VirtIO 块设备的扇区粒度 I/O 转换为 `fatfs` crate 所需的
//! 字节级 `Read`/`Write`/`Seek` 接口。
//!
//! 核心挑战：VirtIO 块设备以 512 字节扇区为单位操作，
//! 而 `fatfs` 需要任意偏移的字节级读写。
//! 解决方案：扇区对齐的 read-modify-write。

use fatfs::{IoBase, IoError, Read, Seek, SeekFrom, Write};

/// 块设备扇区大小（字节）。
pub const SECTOR_SIZE: usize = 512;

/// I/O 适配器错误类型。
#[derive(Debug)]
pub struct BlockIoError;

impl IoError for BlockIoError {
    fn is_interrupted(&self) -> bool {
        false
    }

    fn new_unexpected_eof_error() -> Self {
        Self
    }

    fn new_write_zero_error() -> Self {
        Self
    }
}

impl core::fmt::Display for BlockIoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "BlockIoError")
    }
}

/// VirtIO 块设备的字节级 I/O 适配器——实现 `fatfs` 的 I/O trait。
///
/// 维护当前读写位置（`position`），将字节级操作转换为扇区级操作。
pub struct VirtioBlockAdapter {
    /// 当前字节偏移量
    position: u64,
}

impl VirtioBlockAdapter {
    /// 创建新的适配器，位置初始化为 0。
    pub fn new() -> Self {
        Self { position: 0 }
    }
}

impl Default for VirtioBlockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl IoBase for VirtioBlockAdapter {
    type Error = BlockIoError;
}

impl Read for VirtioBlockAdapter {
    /// 从当前位置读取数据到 `buf`，返回实际读取字节数。
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let blk = crate::device::virtio::virtio_blk().ok_or(BlockIoError)?;
        let mut blk = blk.lock();

        let mut bytes_read = 0;
        let mut remaining = buf;

        while !remaining.is_empty() {
            let sector = self.position / SECTOR_SIZE as u64;
            let offset_in_sector = (self.position % SECTOR_SIZE as u64) as usize;

            let mut sector_buf = [0u8; SECTOR_SIZE];
            blk.read_blocks(sector as usize, &mut sector_buf)
                .map_err(|_| BlockIoError)?;

            let available = SECTOR_SIZE - offset_in_sector;
            let n = remaining.len().min(available);
            remaining[..n].copy_from_slice(&sector_buf[offset_in_sector..offset_in_sector + n]);

            remaining = &mut remaining[n..];
            self.position += n as u64;
            bytes_read += n;
        }

        Ok(bytes_read)
    }
}

impl Write for VirtioBlockAdapter {
    /// 从当前位置写入 `data`，返回实际写入字节数。
    fn write(&mut self, data: &[u8]) -> Result<usize, Self::Error> {
        let blk = crate::device::virtio::virtio_blk().ok_or(BlockIoError)?;
        let mut blk = blk.lock();

        let mut bytes_written = 0;
        let mut remaining = data;

        while !remaining.is_empty() {
            let sector = self.position / SECTOR_SIZE as u64;
            let offset_in_sector = (self.position % SECTOR_SIZE as u64) as usize;

            let mut sector_buf = [0u8; SECTOR_SIZE];

            // Read-modify-write：非对齐写入先读取当前扇区
            if offset_in_sector != 0 || remaining.len() < SECTOR_SIZE {
                blk.read_blocks(sector as usize, &mut sector_buf)
                    .map_err(|_| BlockIoError)?;
            }

            let available = SECTOR_SIZE - offset_in_sector;
            let n = remaining.len().min(available);
            sector_buf[offset_in_sector..offset_in_sector + n].copy_from_slice(&remaining[..n]);

            blk.write_blocks(sector as usize, &sector_buf)
                .map_err(|_| BlockIoError)?;

            remaining = &remaining[n..];
            self.position += n as u64;
            bytes_written += n;
        }

        Ok(bytes_written)
    }

    /// 刷新缓冲区——VirtIO 块设备无缓冲，空操作。
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Seek for VirtioBlockAdapter {
    /// 设置读写位置。
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.position as i64 + offset,
            SeekFrom::End(_) => {
                // 需要知道设备大小——从 VirtIO 块设备获取
                let blk = crate::device::virtio::virtio_blk().ok_or(BlockIoError)?;
                let blk = blk.lock();
                let size = blk.capacity() * SECTOR_SIZE as u64;
                match pos {
                    SeekFrom::End(offset) => size as i64 + offset,
                    _ => unreachable!(),
                }
            }
        };

        if new_pos < 0 {
            return Err(BlockIoError);
        }
        self.position = new_pos as u64;
        Ok(self.position)
    }
}

/// 尝试挂载 FAT 文件系统并执行写入+读回验证。
///
/// 验证流程：
/// 1. 挂载 rootfs.img（FAT32）
/// 2. 创建 `KERNEL_WAS_HERE.TXT`，写入标记内容
/// 3. 关闭文件
/// 4. 重新打开文件，读回全部内容
/// 5. 逐字节比对写入与读出内容
/// 6. 打印内容到串口（供 host 端 `mtools` 二次验证）
///
/// QEMU 退出后 host 可执行：
/// ```sh
/// mtype -i target/.../boot/rootfs.img ::KERNEL_WAS_HERE.TXT
/// ```
pub fn try_mount_fatfs() -> bool {
    if crate::device::virtio::virtio_blk().is_none() {
        log::debug!("FatFS: 无 VirtIO 块设备，跳过挂载");
        return false;
    }

    let adapter = VirtioBlockAdapter::new();
    let fat_fs = match fatfs::FileSystem::new(adapter, fatfs::FsOptions::new()) {
        Ok(fs) => fs,
        Err(e) => {
            log::debug!("FatFS: 挂载失败: {}", e);
            return false;
        }
    };

    log::info!("FatFS: mounted VirtIO block device");

    let write_content = b"SimpleKernel P7 FAT write-read OK\n";
    {
        let root_dir = fat_fs.root_dir();
        let mut file = root_dir
            .create_file("KERNEL_WAS_HERE.TXT")
            .expect("FatFS: create file failed");
        use fatfs::Write;
        file.write_all(write_content).expect("FatFS: write failed");
        file.flush().expect("FatFS: flush failed");
    }
    log::info!(
        "FatFS: wrote {} bytes to KERNEL_WAS_HERE.TXT",
        write_content.len()
    );

    {
        let root_dir = fat_fs.root_dir();
        let mut file = root_dir
            .open_file("KERNEL_WAS_HERE.TXT")
            .expect("FatFS: open file for read failed");
        let mut read_buf = [0u8; 128];
        use fatfs::Read;
        let n = file.read(&mut read_buf).expect("FatFS: read failed");

        // 逐字节比对
        assert_eq!(
            n,
            write_content.len(),
            "FatFS: read size mismatch: read {} bytes, expected {}",
            n,
            write_content.len()
        );
        assert_eq!(
            &read_buf[..n],
            write_content,
            "FatFS: read content mismatch"
        );

        // 打印到串口——host 可搜索此行确认
        let content_str = core::str::from_utf8(&read_buf[..n]).unwrap_or("<non-utf8>");
        log::info!("FatFS: read back: \"{}\"", content_str.trim());
    }

    {
        let root_dir = fat_fs.root_dir();
        let file_count = root_dir.iter().filter_map(|e| e.ok()).count();
        log::info!("FatFS: root dir has {} file(s)", file_count);
    }

    log::info!("=== FatFS WRITE-READ TEST PASSED ===");

    // 保持挂载状态
    core::mem::forget(fat_fs);
    true
}
