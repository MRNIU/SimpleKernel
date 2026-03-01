/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_MMIO_ACCESSOR_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_MMIO_ACCESSOR_HPP_

#include <cstddef>
#include <cstdint>

/**
 * @brief 通用 MMIO 寄存器访问器
 */
struct MmioAccessor {
  explicit MmioAccessor(uint64_t base_addr = 0) : base(base_addr) {}

  template <typename T>
  [[nodiscard]] auto Read(size_t offset) const -> T {
    return *reinterpret_cast<volatile T*>(base + offset);
  }

  template <typename T>
  auto Write(size_t offset, T val) const -> void {
    *reinterpret_cast<volatile T*>(base + offset) = val;
  }

  uint64_t base;
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_MMIO_ACCESSOR_HPP_
