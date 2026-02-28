/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_

#include <cstddef>
#include <cstdint>
#include <span>

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

 private:
  /// 缓冲区数据指针
  uint8_t* data_{nullptr};
  /// 缓冲区大小
  size_t size_{0};
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_
