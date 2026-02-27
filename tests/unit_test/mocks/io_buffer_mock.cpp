/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief IoBuffer mock for unit tests — uses malloc/free instead of
 *        aligned_alloc which requires the memory subsystem.
 */

#include <cstdlib>
#include <cstring>
#include <utility>

#include "io_buffer.hpp"

IoBuffer::IoBuffer(size_t size, size_t /*alignment*/)
    : buffer_{static_cast<uint8_t*>(malloc(size)), size} {
  if (buffer_.first != nullptr) {
    memset(buffer_.first, 0, size);
  }
}

IoBuffer::~IoBuffer() {
  free(buffer_.first);
  buffer_ = {nullptr, 0};
}

IoBuffer::IoBuffer(IoBuffer&& other) noexcept
    : buffer_{std::exchange(other.buffer_, {nullptr, 0})} {}

auto IoBuffer::operator=(IoBuffer&& other) noexcept -> IoBuffer& {
  if (this != &other) {
    free(buffer_.first);
    buffer_ = std::exchange(other.buffer_, {nullptr, 0});
  }
  return *this;
}

auto IoBuffer::GetBuffer() const -> std::pair<const uint8_t*, size_t> {
  return buffer_;
}

auto IoBuffer::GetBuffer() -> std::pair<uint8_t*, size_t> { return buffer_; }

auto IoBuffer::IsValid() const -> bool { return buffer_.first != nullptr; }
