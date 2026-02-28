/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Virtio bottom-half messages for etl::message_router
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_VIRTIO_MESSAGES_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_VIRTIO_MESSAGES_HPP_

#include <etl/message.h>

#include <cstdint>

/// @brief Virtio message IDs (start at 200 to avoid collision)
namespace virtio_msg_id {
inline constexpr etl::message_id_t kQueueReady = 200;
}  // namespace virtio_msg_id

/// @brief Virtio queue ready notification (sent by top-half ISR)
struct VirtioQueueReadyMsg : public etl::message<virtio_msg_id::kQueueReady> {
  uint32_t queue_index;
  explicit VirtioQueueReadyMsg(uint32_t idx) : queue_index(idx) {}
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_VIRTIO_MESSAGES_HPP_
