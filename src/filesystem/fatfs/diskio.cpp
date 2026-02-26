/**
 * @copyright Copyright The SimpleKernel Contributors
 * @file    diskio.cpp
 * @brief   FatFs low-level disk I/O module implementation for SimpleKernel
 * @details This file provides the disk I/O interface required by FatFs.
 *          It must be implemented by the user to interface with the storage
 * device.
 */

#include "diskio.h"

#include "ff.h"

/**
 * @brief  Get drive status
 * @param  pdrv Physical drive number to identify the drive
 * @return Drive status flags
 * @note   This function is called by FatFs to check the status of the physical
 * drive.
 */
DSTATUS disk_status(BYTE pdrv) {
  // TODO: Implement drive status check
  // Return STA_NOINIT if the drive is not initialized
  // Return 0 if the drive is ready
  // Return STA_NODISK if no medium in the drive
  // Return STA_PROTECT if the medium is write protected

  switch (pdrv) {
    case 0:  // RAM disk
      // TODO: Check RAM disk status
      return STA_NOINIT;
    case 1:  // SD card
      // TODO: Check SD card status
      return STA_NOINIT;
    default:
      return STA_NOINIT;
  }
}

/**
 * @brief  Initialize a drive
 * @param  pdrv Physical drive number to identify the drive
 * @return Drive status flags
 * @note   This function is called by FatFs to initialize the storage device.
 */
DSTATUS disk_initialize(BYTE pdrv) {
  // TODO: Implement drive initialization
  // Return 0 on success, or STA_NOINIT on failure

  switch (pdrv) {
    case 0:  // RAM disk
      // TODO: Initialize RAM disk
      return STA_NOINIT;
    case 1:  // SD card
      // TODO: Initialize SD card
      return STA_NOINIT;
    default:
      return STA_NOINIT;
  }
}

/**
 * @brief  Read sector(s)
 * @param  pdrv   Physical drive number to identify the drive
 * @param  buff   Data buffer to store read data
 * @param  sector Start sector in LBA
 * @param  count  Number of sectors to read
 * @return Result code
 * @note   This function is called by FatFs to read data from the storage
 * device.
 */
DRESULT disk_read(BYTE pdrv, BYTE* buff, LBA_t sector, UINT count) {
  // TODO: Implement sector read
  // Return RES_OK on success
  // Return RES_ERROR on R/W error
  // Return RES_PARERR on invalid parameter

  switch (pdrv) {
    case 0:  // RAM disk
      // TODO: Read from RAM disk
      (void)buff;
      (void)sector;
      (void)count;
      return RES_PARERR;
    case 1:  // SD card
      // TODO: Read from SD card
      (void)buff;
      (void)sector;
      (void)count;
      return RES_PARERR;
    default:
      return RES_PARERR;
  }
}

#if FF_FS_READONLY == 0

/**
 * @brief  Write sector(s)
 * @param  pdrv   Physical drive number to identify the drive
 * @param  buff   Data to be written
 * @param  sector Start sector in LBA
 * @param  count  Number of sectors to write
 * @return Result code
 * @note   This function is called by FatFs to write data to the storage device.
 */
DRESULT disk_write(BYTE pdrv, const BYTE* buff, LBA_t sector, UINT count) {
  // TODO: Implement sector write
  // Return RES_OK on success
  // Return RES_ERROR on R/W error
  // Return RES_WRPRT on write protected
  // Return RES_PARERR on invalid parameter

  switch (pdrv) {
    case 0:  // RAM disk
      // TODO: Write to RAM disk
      (void)buff;
      (void)sector;
      (void)count;
      return RES_PARERR;
    case 1:  // SD card
      // TODO: Write to SD card
      (void)buff;
      (void)sector;
      (void)count;
      return RES_PARERR;
    default:
      return RES_PARERR;
  }
}

#endif  // FF_FS_READONLY == 0

/**
 * @brief  Miscellaneous functions
 * @param  pdrv Physical drive number (0..)
 * @param  cmd  Control code
 * @param  buff Buffer to send/receive control data
 * @return Result code
 * @note   This function is called by FatFs to control the storage device.
 *         Required commands:
 *         - CTRL_SYNC: Complete pending write process
 *         - GET_SECTOR_COUNT: Get media size (needed if FF_USE_MKFS == 1)
 *         - GET_SECTOR_SIZE: Get sector size (needed if FF_MAX_SS != FF_MIN_SS)
 *         - GET_BLOCK_SIZE: Get erase block size (needed if FF_USE_MKFS == 1)
 *         - CTRL_TRIM: Inform device that the data on the block is no longer
 * used
 */
DRESULT disk_ioctl(BYTE pdrv, BYTE cmd, void* buff) {
  // TODO: Implement miscellaneous disk control
  // Return RES_OK on success
  // Return RES_PARERR on invalid parameter
  // Return RES_NOTRDY if drive not ready

  switch (pdrv) {
    case 0:  // RAM disk
      switch (cmd) {
        case CTRL_SYNC:
          // No cache to sync for RAM disk
          return RES_OK;
        case GET_SECTOR_COUNT:
          // TODO: Return RAM disk sector count
          *(LBA_t*)buff = 0;
          return RES_OK;
        case GET_SECTOR_SIZE:
          // TODO: Return RAM disk sector size
          *(WORD*)buff = 512;
          return RES_OK;
        case GET_BLOCK_SIZE:
          // TODO: Return RAM disk erase block size
          *(DWORD*)buff = 1;
          return RES_OK;
        case CTRL_TRIM:
          // No TRIM support for RAM disk
          return RES_OK;
        default:
          return RES_PARERR;
      }
    case 1:  // SD card
      switch (cmd) {
        case CTRL_SYNC:
          // TODO: Sync SD card cache
          return RES_OK;
        case GET_SECTOR_COUNT:
          // TODO: Return SD card sector count
          *(LBA_t*)buff = 0;
          return RES_OK;
        case GET_SECTOR_SIZE:
          // TODO: Return SD card sector size
          *(WORD*)buff = 512;
          return RES_OK;
        case GET_BLOCK_SIZE:
          // TODO: Return SD card erase block size
          *(DWORD*)buff = 1;
          return RES_OK;
        case CTRL_TRIM:
          // TODO: Implement TRIM for SD card
          return RES_OK;
        default:
          return RES_PARERR;
      }
    default:
      return RES_PARERR;
  }
}
