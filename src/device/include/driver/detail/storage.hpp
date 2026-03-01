/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_STORAGE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_STORAGE_HPP_

#include <cstdint>
#include <utility>

namespace detail {

/**
 * @brief Freestanding-compatible placement-new optional storage
 *
 * Provides aligned storage for a single value of type T without requiring
 * heap allocation. Suitable for freestanding/bare-metal environments where
 * std::optional may pull in unwanted dependencies.
 *
 * @tparam T Value type to store
 *
 * @pre  T must be move-constructible
 * @post HasValue() returns true only after a successful Emplace()
 */
template <typename T>
class Storage {
 public:
  Storage() = default;
  ~Storage() { Reset(); }

  Storage(const Storage&) = delete;
  auto operator=(const Storage&) -> Storage& = delete;

  Storage(Storage&& other) noexcept : initialized_(other.initialized_) {
    if (initialized_) {
      new (buf_) T(std::move(*other.Ptr()));
      other.Reset();
    }
  }

  auto operator=(Storage&& other) noexcept -> Storage& {
    if (this != &other) {
      Reset();
      if (other.initialized_) {
        new (buf_) T(std::move(*other.Ptr()));
        initialized_ = true;
        other.Reset();
      }
    }
    return *this;
  }

  /**
   * @brief Construct a T in-place
   *
   * @pre HasValue() == false
   * @post HasValue() == true
   */
  template <typename... Args>
  auto Emplace(Args&&... args) -> T& {
    Reset();
    new (buf_) T(static_cast<Args&&>(args)...);
    initialized_ = true;
    return *Ptr();
  }

  /**
   * @brief Destroy the stored value (if any)
   *
   * @post HasValue() == false
   */
  auto Reset() -> void {
    if (initialized_) {
      Ptr()->~T();
      initialized_ = false;
    }
  }

  /// @return true if a value is currently stored
  [[nodiscard]] auto HasValue() const -> bool { return initialized_; }

  /// @pre HasValue() == true
  [[nodiscard]] auto Value() -> T& { return *Ptr(); }
  [[nodiscard]] auto Value() const -> const T& { return *Ptr(); }

 private:
  [[nodiscard]] auto Ptr() -> T* {
    return reinterpret_cast<T*>(
        buf_);  // NOLINT(cppcoreguidelines-pro-type-reinterpret-cast)
  }
  [[nodiscard]] auto Ptr() const -> const T* {
    return reinterpret_cast<const T*>(
        buf_);  // NOLINT(cppcoreguidelines-pro-type-reinterpret-cast)
  }

  alignas(T) uint8_t buf_[sizeof(T)] = {};
  bool initialized_ = false;
};

}  // namespace detail

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_STORAGE_HPP_
