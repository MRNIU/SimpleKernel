/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_PROVIDER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_PROVIDER_HPP_

#include "block_device.hpp"

/**
 * @brief Returns the kernel vfs::BlockDevice* for the first probed virtio-blk
 *        device, or nullptr if no device has been probed yet.
 *
 * @pre  DeviceInit() has been called and VirtioBlkDriver probe succeeded.
 * @post Returns a pointer to a static adapter object whose lifetime is the
 *       kernel lifetime. The pointer must not be deleted.
 */
auto GetVirtioBlkBlockDevice() -> vfs::BlockDevice*;

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_PROVIDER_HPP_ */
