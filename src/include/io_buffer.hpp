/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_

#include <cstddef>
#include <cstdint>
#include <utility>

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

  /**
   * @brief 析构函数
   */
  ~IoBuffer();
  /// @}

  /// @name 拷贝与移动控制
  /// @{
  // Disable copy
  IoBuffer(const IoBuffer&) = delete;
  auto operator=(const IoBuffer&) -> IoBuffer& = delete;

  // Enable move
  IoBuffer(IoBuffer&& other) noexcept;
  auto operator=(IoBuffer&& other) noexcept -> IoBuffer&;
  /// @}

  /**
   * @brief 获取缓冲区数据与大小 (只读)
   * @return std::pair<const uint8_t*, size_t> 数据指针与大小的 pair
   * @pre None
   * @post 返回指向缓冲区数据的常量指针与大小
   */
  [[nodiscard]] auto GetBuffer() const -> std::pair<const uint8_t*, size_t>;

  /**
   * @brief 获取缓冲区数据与大小
   * @return std::pair<uint8_t*, size_t> 数据指针与大小的 pair
   * @pre None
   * @post 返回指向缓冲区数据的指针与大小
   */
  [[nodiscard]] auto GetBuffer() -> std::pair<uint8_t*, size_t>;

  /**
   * @brief 检查缓冲区是否有效
   * @return bool 有效则返回 true
   * @pre None
   * @post 返回缓冲区是否已分配且有效的状态
   */
  [[nodiscard]] auto IsValid() const -> bool;

 private:
  /// 缓冲区数据与大小
  std::pair<uint8_t*, size_t> buffer_{nullptr, 0};
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_ */
