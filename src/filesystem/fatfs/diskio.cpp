/**
 * @copyright Copyright The SimpleKernel Contributors
 * @file    diskio.cpp
 * @brief   FatFs 底层磁盘 I/O — 委托给 vfs::BlockDevice
 */

// ff.h 必须在 diskio.h 之前包含，因为 diskio.h 中使用的 BYTE/LBA_t 等类型
// 均由 ff.h 定义。
// clang-format off
#include "ff.h"
#include "diskio.h"
// clang-format on

#include "fatfs.hpp"
#include "kernel_log.hpp"

/// get_fattime — 返回 FAT 时间戳。无 RTC 时返回 0（epoch）。
/// FF_FS_READONLY == 0 时 FatFS 要求此函数存在。
extern "C" DWORD get_fattime() { return 0; }

// ── disk_status ──────────────────────────────────────────────────────────────

DSTATUS disk_status(BYTE pdrv) {
  if (fatfs::FatFsFileSystem::GetBlockDevice(pdrv) == nullptr) {
    return STA_NOINIT;
  }
  return 0;  // 驱动器就绪
}

// ── disk_initialize ──────────────────────────────────────────────────────────

DSTATUS disk_initialize(BYTE pdrv) {
  if (fatfs::FatFsFileSystem::GetBlockDevice(pdrv) == nullptr) {
    return STA_NOINIT;
  }
  return 0;  // BlockDevice 已由调用方初始化
}

// ── disk_read ────────────────────────────────────────────────────────────────

DRESULT disk_read(BYTE pdrv, BYTE* buff, LBA_t sector, UINT count) {
  auto* dev = fatfs::FatFsFileSystem::GetBlockDevice(pdrv);
  if (dev == nullptr) {
    return RES_NOTRDY;
  }
  auto result = dev->ReadSectors(sector, static_cast<uint32_t>(count), buff);
  if (!result) {
    klog::Err("disk_read: pdrv=%u sector=%llu count=%u failed\n",
              static_cast<unsigned>(pdrv),
              static_cast<unsigned long long>(sector),
              static_cast<unsigned>(count));
    return RES_ERROR;
  }
  return RES_OK;
}

// ── disk_write ───────────────────────────────────────────────────────────────

#if FF_FS_READONLY == 0

DRESULT disk_write(BYTE pdrv, const BYTE* buff, LBA_t sector, UINT count) {
  auto* dev = fatfs::FatFsFileSystem::GetBlockDevice(pdrv);
  if (dev == nullptr) {
    return RES_NOTRDY;
  }
  auto result = dev->WriteSectors(sector, static_cast<uint32_t>(count), buff);
  if (!result) {
    klog::Err("disk_write: pdrv=%u sector=%llu count=%u failed\n",
              static_cast<unsigned>(pdrv),
              static_cast<unsigned long long>(sector),
              static_cast<unsigned>(count));
    return RES_ERROR;
  }
  return RES_OK;
}

#endif  // FF_FS_READONLY == 0

// ── disk_ioctl ───────────────────────────────────────────────────────────────

DRESULT disk_ioctl(BYTE pdrv, BYTE cmd, void* buff) {
  auto* dev = fatfs::FatFsFileSystem::GetBlockDevice(pdrv);
  if (dev == nullptr) {
    return RES_NOTRDY;
  }
  switch (cmd) {
    case CTRL_SYNC: {
      auto r = dev->Flush();
      return r ? RES_OK : RES_ERROR;
    }
    case GET_SECTOR_COUNT:
      static_cast<LBA_t*>(buff)[0] = static_cast<LBA_t>(dev->GetSectorCount());
      return RES_OK;
    case GET_SECTOR_SIZE:
      static_cast<WORD*>(buff)[0] = static_cast<WORD>(dev->GetSectorSize());
      return RES_OK;
    case GET_BLOCK_SIZE:
      static_cast<DWORD*>(buff)[0] = 1;
      return RES_OK;
    case CTRL_TRIM:
      return RES_OK;  // TRIM 不是必需的
    default:
      return RES_PARERR;
  }
}
