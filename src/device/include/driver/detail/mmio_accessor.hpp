/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_MMIO_ACCESSOR_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_MMIO_ACCESSOR_HPP_

#include <cstddef>
#include <cstdint>

namespace detail {

/**
 * @brief 通用 MMIO 寄存器访问器
 *
 * 封装 volatile 指针的 MMIO 读写操作，消除各驱动中重复的
 * reinterpret_cast<volatile T*> 样板代码。
 *
 * 支持任意宽度的寄存器访问（uint8_t / uint16_t / uint32_t / uint64_t）。
 */
class MmioAccessor {
 public:
  explicit MmioAccessor(uint64_t base = 0) : base_(base) {}

  template <typename T>
  [[nodiscard]] auto Read(size_t offset) const -> T {
    return *reinterpret_cast<volatile T*>(base_ + offset);
  }

  template <typename T>
  auto Write(size_t offset, T val) const -> void {
    *reinterpret_cast<volatile T*>(base_ + offset) = val;
  }

  [[nodiscard]] auto base() const -> uint64_t { return base_; }

 private:
  uint64_t base_;
};

}  // namespace detail

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_MMIO_ACCESSOR_HPP_
