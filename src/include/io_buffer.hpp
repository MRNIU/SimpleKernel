/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_

#include <cstddef>
#include <cstdint>
#include <span>

#include "expected.hpp"

/// @brief Address translation callback type
/// @param virt Virtual address to translate
/// @return Corresponding physical address
using VirtToPhysFunc = auto (*)(uintptr_t virt) -> uintptr_t;

/// @brief Identity mapping: phys == virt (default for early boot / no MMU)
[[nodiscard]] inline auto IdentityVirtToPhys(uintptr_t virt) -> uintptr_t {
  return virt;
}

/**
 * @brief Non-owning descriptor for a DMA-accessible memory region
 *
 * Bundles virtual address, physical (bus) address, and size into a single
 * value type. Does NOT own the memory — lifetime is managed by the allocator
 * (e.g. IoBuffer).
 *
 * @note POD-like, trivially copyable, safe to pass by value or const-ref.
 */
struct DmaRegion {
  /// Virtual (CPU-accessible) base address
  void* virt{nullptr};
  /// Physical (bus/DMA) base address
  uintptr_t phys{0};
  /// Region size in bytes
  size_t size{0};

  /// @brief Check if the region is valid (non-null, non-zero size)
  [[nodiscard]] auto IsValid() const -> bool {
    return virt != nullptr && size > 0;
  }

  /// @brief Get typed pointer to the virtual base
  [[nodiscard]] auto data() const -> uint8_t* {
    return static_cast<uint8_t*>(virt);
  }

  /**
   * @brief Create a sub-region at the given offset
   *
   * @param offset Byte offset from the start of this region
   * @param len Byte length of the sub-region
   * @return Sub-region on success, error if out of bounds
   *
   * @pre offset + len <= size
   * @post Returned region shares the same underlying memory
   */
  [[nodiscard]] auto SubRegion(size_t offset, size_t len) const
      -> Expected<DmaRegion> {
    if (offset + len > size) {
      return std::unexpected(Error{ErrorCode::kInvalidArgument});
    }
    return DmaRegion{
        .virt = static_cast<uint8_t*>(virt) + offset,
        .phys = phys + offset,
        .size = len,
    };
  }
};

/**
 * @brief RAII wrapper for dynamically allocated, aligned IO buffers.
 *
 * @pre  None
 * @post The buffer memory is correctly allocated and aligned. It will be freed
 * upon destruction.
 */
class IoBuffer {
 public:
  /// Default alignment for IO buffers (e.g., page size)
  static constexpr size_t kDefaultAlignment = 4096;

  /// @name 构造/析构函数
  /// @{
  IoBuffer() = default;

  /**
   * @brief 构造函数
   * @param  size 缓冲区大小
   * @param  alignment 对齐要求
   * @pre size > 0, alignment 必须是 2 的幂
   * @post 缓冲区内存已正确分配并对齐
   */
  IoBuffer(size_t size, size_t alignment = kDefaultAlignment);

  ~IoBuffer();

  IoBuffer(const IoBuffer&) = delete;
  auto operator=(const IoBuffer&) -> IoBuffer& = delete;

  IoBuffer(IoBuffer&& other);
  auto operator=(IoBuffer&& other) noexcept -> IoBuffer&;
  /// @}

  /**
   * @brief 获取缓冲区数据与大小 (只读)
   * @return std::span<const uint8_t> 缓冲区数据的只读视图
   * @pre None
   * @post 返回指向缓冲区数据的常量 span
   */
  [[nodiscard]] auto GetBuffer() const -> std::span<const uint8_t>;

  /**
   * @brief 获取缓冲区数据与大小
   * @return std::span<uint8_t> 缓冲区数据的可变视图
   * @pre None
   * @post 返回指向缓冲区数据的 span
   */
  [[nodiscard]] auto GetBuffer() -> std::span<uint8_t>;

  /**
   * @brief 检查缓冲区是否有效
   * @return bool 有效则返回 true
   * @pre None
   * @post 返回缓冲区是否已分配且有效的状态
   */
  [[nodiscard]] auto IsValid() const -> bool;

  /**
   * @brief Create a DmaRegion view of this buffer
   *
   * @param v2p Address translation function (default: identity mapping)
   * @return DmaRegion describing this buffer's memory
   *
   * @pre IsValid() == true
   * @post Returned DmaRegion does not own the memory
   */
  [[nodiscard]] auto ToDmaRegion(VirtToPhysFunc v2p = IdentityVirtToPhys) const
      -> DmaRegion;

 private:
  /// 缓冲区数据指针
  uint8_t* data_{nullptr};
  /// 缓冲区大小
  size_t size_{0};
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_
