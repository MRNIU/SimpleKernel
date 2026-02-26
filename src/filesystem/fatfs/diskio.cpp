/**
 * @copyright Copyright The SimpleKernel Contributors
 * @file    diskio.cpp
 * @brief   FatFs low-level disk I/O — delegates to vfs::BlockDevice
 */

// ff.h must be included before diskio.h because diskio.h uses BYTE/LBA_t/etc.
// which are defined only after ff.h has been processed.
#include "diskio.h"

#include "block_device.hpp"
#include "ff.h"
#include "kernel_log.hpp"

/// Per-volume block device registry.  Set by FatFsFileSystem::Mount().
/// Indexed by FatFS physical drive number (pdrv == volume_id).
vfs::BlockDevice* g_block_devices[FF_VOLUMES] = {};
/// get_fattime — returns FAT timestamp. Returns 0 (epoch) as we have no RTC.
/// Required by FatFS when FF_FS_READONLY == 0.
extern "C" DWORD get_fattime() { return 0; }

// ── disk_status ──────────────────────────────────────────────────────────────

DSTATUS disk_status(BYTE pdrv) {
  if (pdrv >= FF_VOLUMES || g_block_devices[pdrv] == nullptr) {
    return STA_NOINIT;
  }
  return 0;  // drive ready
}

// ── disk_initialize ──────────────────────────────────────────────────────────

DSTATUS disk_initialize(BYTE pdrv) {
  if (pdrv >= FF_VOLUMES || g_block_devices[pdrv] == nullptr) {
    return STA_NOINIT;
  }
  return 0;  // already initialized via BlockDevice
}

// ── disk_read ────────────────────────────────────────────────────────────────

DRESULT disk_read(BYTE pdrv, BYTE* buff, LBA_t sector, UINT count) {
  if (pdrv >= FF_VOLUMES || g_block_devices[pdrv] == nullptr) {
    return RES_NOTRDY;
  }
  auto result = g_block_devices[pdrv]->ReadSectors(
      sector, static_cast<uint32_t>(count), buff);
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
  if (pdrv >= FF_VOLUMES || g_block_devices[pdrv] == nullptr) {
    return RES_NOTRDY;
  }
  auto result = g_block_devices[pdrv]->WriteSectors(
      sector, static_cast<uint32_t>(count), buff);
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
  if (pdrv >= FF_VOLUMES || g_block_devices[pdrv] == nullptr) {
    return RES_NOTRDY;
  }
  vfs::BlockDevice* dev = g_block_devices[pdrv];
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
      return RES_OK;  // TRIM not required
    default:
      return RES_PARERR;
  }
}
