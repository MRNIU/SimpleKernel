/**
 * @copyright Copyright The MPMCQueue Contributors
 */

#ifndef MPMCQUEUE_INCLUDE_MPMCQUEUE_HPP_
#define MPMCQUEUE_INCLUDE_MPMCQUEUE_HPP_

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <type_traits>
#include <utility>

namespace mpmc_queue {

/**
 * @brief Multi-Producer Multi-Consumer Lock-Free Queue
 *
 * This implementation uses a ring buffer with atomic operations to provide
 * lock-free enqueue and dequeue operations. It's suitable for freestanding
 * environments as it doesn't use dynamic memory allocation, exceptions, or
 * other standard library facilities beyond atomics.
 *
 * @tparam T The type of elements stored in the queue
 * @tparam Capacity The maximum number of elements (must be power of 2)
 */
template <typename T, size_t Capacity>
class MPMCQueue {
  static_assert((Capacity & (Capacity - 1)) == 0,
                "Capacity must be a power of 2");
  static_assert(Capacity > 0, "Capacity must be greater than 0");

 public:
  using value_type = T;
  using size_type = size_t;
  using reference = T&;
  using const_reference = const T&;

  /**
   * @brief Construct a new MPMCQueue object
   */
  constexpr MPMCQueue() noexcept : head_(0), tail_(0) {
    for (size_t i = 0; i < Capacity; ++i) {
      buffer_[i].sequence.store(i, std::memory_order_relaxed);
    }
  }

  /**
   * @brief Destroy the MPMCQueue object
   */
  ~MPMCQueue() noexcept = default;

  MPMCQueue(const MPMCQueue&) = delete;
  auto operator=(const MPMCQueue&) -> MPMCQueue& = delete;
  MPMCQueue(MPMCQueue&&) = delete;
  auto operator=(MPMCQueue&&) -> MPMCQueue& = delete;

  /**
   * @brief Attempt to enqueue an item
   *
   * @param item The item to enqueue
   * @return true if the item was successfully enqueued
   * @return false if the queue is full
   */
  [[nodiscard]] auto push(const T& item) noexcept -> bool {
    return enqueue_impl(item);
  }

  /**
   * @brief Attempt to enqueue an item (move version)
   *
   * @param item The item to enqueue
   * @return true if the item was successfully enqueued
   * @return false if the queue is full
   */
  [[nodiscard]] auto push(T&& item) noexcept -> bool {
    return enqueue_impl(std::move(item));
  }

  /**
   * @brief Attempt to dequeue an item
   *
   * @param item Reference to store the dequeued item
   * @return true if an item was successfully dequeued
   * @return false if the queue is empty
   */
  [[nodiscard]] auto pop(T& item) noexcept -> bool {
    size_t pos;
    Cell* cell;
    size_t seq;

    pos = tail_.load(std::memory_order_relaxed);

    for (;;) {
      cell = &buffer_[pos & (Capacity - 1)];
      seq = cell->sequence.load(std::memory_order_acquire);
      intptr_t diff =
          static_cast<intptr_t>(seq) - static_cast<intptr_t>(pos + 1);

      if (diff == 0) {
        if (tail_.compare_exchange_weak(pos, pos + 1,
                                        std::memory_order_relaxed)) {
          item = std::move(cell->data);
          cell->sequence.store(pos + Capacity, std::memory_order_release);
          return true;
        }
      } else if (diff < 0) {
        return false;
      } else {
        pos = tail_.load(std::memory_order_relaxed);
      }
    }
  }

  /**
   * @brief Get the capacity of the queue
   *
   * @return constexpr size_t The maximum number of elements
   */
  [[nodiscard]] static constexpr auto max_size() noexcept -> size_t {
    return Capacity;
  }

  /**
   * @brief Get an approximate size of the queue
   *
   * Note: This is an approximation and may not be accurate in concurrent
   * scenarios.
   *
   * @return size_t Approximate number of elements in the queue
   */
  [[nodiscard]] auto size() const noexcept -> size_t {
    size_t head = head_.load(std::memory_order_relaxed);
    size_t tail = tail_.load(std::memory_order_relaxed);
    return head >= tail ? head - tail : 0;
  }

  /**
   * @brief Check if the queue is empty (approximate)
   *
   * Note: This is an approximation and may not be accurate in concurrent
   * scenarios.
   *
   * @return true if the queue appears to be empty
   * @return false if the queue appears to have elements
   */
  [[nodiscard]] auto empty() const noexcept -> bool { return size() == 0; }

 private:
  struct Cell {
    std::atomic<size_t> sequence;
    T data;
  };

  template <typename U>
  [[nodiscard]] auto enqueue_impl(U&& item) noexcept -> bool {
    size_t pos;
    Cell* cell;
    size_t seq;

    pos = head_.load(std::memory_order_relaxed);

    for (;;) {
      cell = &buffer_[pos & (Capacity - 1)];
      seq = cell->sequence.load(std::memory_order_acquire);
      intptr_t diff = static_cast<intptr_t>(seq) - static_cast<intptr_t>(pos);

      if (diff == 0) {
        if (head_.compare_exchange_weak(pos, pos + 1,
                                        std::memory_order_relaxed)) {
          cell->data = std::forward<U>(item);
          cell->sequence.store(pos + 1, std::memory_order_release);
          return true;
        }
      } else if (diff < 0) {
        return false;
      } else {
        pos = head_.load(std::memory_order_relaxed);
      }
    }
  }

  // Cache line padding to avoid false sharing
  static constexpr size_t kCacheLineSize = 64;

  alignas(kCacheLineSize) std::atomic<size_t> head_;
  alignas(kCacheLineSize) std::atomic<size_t> tail_;

  // Ring buffer
  alignas(kCacheLineSize) Cell buffer_[Capacity];
};

}  // namespace mpmc_queue

#endif  // MPMCQUEUE_INCLUDE_MPMCQUEUE_HPP_
