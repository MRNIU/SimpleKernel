/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "io_buffer.hpp"

#include "kernel_log.hpp"
#include "sk_stdlib.h"

IoBuffer::IoBuffer(size_t size, size_t alignment) {
  if (size == 0 || alignment == 0 || (alignment & (alignment - 1)) != 0) {
    klog::Err("IoBuffer: invalid size (%lu) or alignment (%lu)\n", size,
              alignment);
    buffer_ = {nullptr, 0};
    return;
  }

  // Use aligned_alloc provided by bmalloc / sk_stdlib.h
  uint8_t* data = static_cast<uint8_t*>(aligned_alloc(alignment, size));
  if (data == nullptr) {
    klog::Err("IoBuffer: aligned_alloc failed for size %lu, alignment %lu\n",
              size, alignment);
    buffer_ = {nullptr, 0};
  } else {
    buffer_ = {data, size};
  }
}

IoBuffer::~IoBuffer() {
  if (buffer_.first != nullptr) {
    aligned_free(buffer_.first);
    buffer_ = {nullptr, 0};
  }
}

IoBuffer::IoBuffer(IoBuffer&& other) noexcept : buffer_(other.buffer_) {
  other.buffer_ = {nullptr, 0};
}

auto IoBuffer::operator=(IoBuffer&& other) noexcept -> IoBuffer& {
  if (this != &other) {
    if (buffer_.first != nullptr) {
      aligned_free(buffer_.first);
    }
    buffer_ = other.buffer_;
    other.buffer_ = {nullptr, 0};
  }
  return *this;
}

auto IoBuffer::GetBuffer() const -> std::pair<const uint8_t*, size_t> {
  return {buffer_.first, buffer_.second};
}

auto IoBuffer::GetBuffer() -> std::pair<uint8_t*, size_t> { return buffer_; }

auto IoBuffer::IsValid() const -> bool { return buffer_.first != nullptr; }
