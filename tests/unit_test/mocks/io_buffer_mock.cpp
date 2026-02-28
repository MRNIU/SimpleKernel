/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief IoBuffer mock for unit tests — uses malloc/free instead of
 *        aligned_alloc which requires the memory subsystem.
 */

#include <cstdlib>
#include <cstring>
#include <span>
#include <utility>

#include "io_buffer.hpp"

IoBuffer::IoBuffer(size_t size, size_t /*alignment*/)
    : data_{static_cast<uint8_t*>(malloc(size))}, size_{size} {
  if (data_ != nullptr) {
    memset(data_, 0, size);
  }
}

IoBuffer::~IoBuffer() {
  free(data_);
  data_ = nullptr;
  size_ = 0;
}

IoBuffer::IoBuffer(IoBuffer&& other)
    : data_{std::exchange(other.data_, nullptr)},
      size_{std::exchange(other.size_, 0)} {}

auto IoBuffer::operator=(IoBuffer&& other) noexcept -> IoBuffer& {
  if (this != &other) {
    free(data_);
    data_ = std::exchange(other.data_, nullptr);
    size_ = std::exchange(other.size_, 0);
  }
  return *this;
}

auto IoBuffer::GetBuffer() const -> std::span<const uint8_t> {
  return {data_, size_};
}

auto IoBuffer::GetBuffer() -> std::span<uint8_t> { return {data_, size_}; }

auto IoBuffer::IsValid() const -> bool { return data_ != nullptr; }
