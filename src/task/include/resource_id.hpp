/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_RESOURCE_ID_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_RESOURCE_ID_HPP_

#include <cstdint>
#include <functional>

#include "etl/hash.h"

/**
 * @brief 资源类型枚举
 */
enum class ResourceType : uint8_t {
  // 无效资源
  kNone = 0x00,
  // 互斥锁
  kMutex = 0x01,
  // 信号量
  kSemaphore = 0x02,
  // 条件变量
  kCondVar = 0x03,
  // 等待子进程退出
  kChildExit = 0x04,
  // IO 完成
  kIoComplete = 0x05,
  // Futex (快速用户空间互斥锁)
  kFutex = 0x06,
  // 信号
  kSignal = 0x07,
  // 定时器
  kTimer = 0x08,
  // 中断（用于中断线程化）
  kInterrupt = 0x09,
  // 可以继续扩展...
  kResourceTypeCount,
};

/**
 * @brief 获取资源类型的字符串表示（用于调试）
 * @param type 资源类型
 * @return 类型名称字符串
 */
constexpr const char* GetResourceTypeName(ResourceType type) {
  switch (type) {
    case ResourceType::kNone:
      return "None";
    case ResourceType::kMutex:
      return "Mutex";
    case ResourceType::kSemaphore:
      return "Semaphore";
    case ResourceType::kCondVar:
      return "CondVar";
    case ResourceType::kChildExit:
      return "ChildExit";
    case ResourceType::kIoComplete:
      return "IoComplete";
    case ResourceType::kFutex:
      return "Futex";
    case ResourceType::kSignal:
      return "Signal";
    case ResourceType::kTimer:
      return "Timer";
    default:
      return "Unknown";
  }
}

/**
 * @brief 资源 ID
 *
 * [63:56] - 资源类型 (8 bits)
 * [55:0]  - 资源数据 (56 bits)
 */
struct ResourceId {
  /// 获取资源类型
  constexpr ResourceType GetType() const {
    return static_cast<ResourceType>((value_ >> kTypeShift) & 0xFF);
  }

  /// 获取资源数据
  constexpr uint64_t GetData() const { return value_ & kDataMask; }

  /// 获取类型名称（用于调试）
  constexpr const char* GetTypeName() const {
    return GetResourceTypeName(GetType());
  }

  /// 检查是否为无效资源
  constexpr explicit operator bool() const {
    return GetType() != ResourceType::kNone;
  }

  /// 隐式转换到 uint64_t（用于存储和比较）
  constexpr operator uint64_t() const { return value_; }

  /// 比较操作符
  constexpr bool operator==(const ResourceId& other) const {
    return value_ == other.value_;
  }

  constexpr bool operator!=(const ResourceId& other) const {
    return value_ != other.value_;
  }

  /**
   * @brief 构造资源 ID
   * @param type 资源类型
   * @param data 资源数据 (如地址、PID 等)
   */
  constexpr ResourceId(ResourceType type, uint64_t data)
      : value_((static_cast<uint64_t>(type) << kTypeShift) |
               (data & kDataMask)) {}

  /// @name 构造/析构函数
  /// @{
  ResourceId() = default;
  ResourceId(const ResourceId&) = default;
  ResourceId(ResourceId&&) = default;
  auto operator=(const ResourceId&) -> ResourceId& = default;
  auto operator=(ResourceId&&) -> ResourceId& = default;
  ~ResourceId() = default;
  /// @}

 private:
  static constexpr uint8_t kTypeShift = 56;
  static constexpr uint64_t kTypeMask = 0xFF00000000000000ULL;
  static constexpr uint64_t kDataMask = 0x00FFFFFFFFFFFFFFULL;

  // 内部存储的原始值
  uint64_t value_;
};

// std::hash 特化，使 ResourceId 可以作为 unordered_map 的键
namespace std {
template <>
struct hash<ResourceId> {
  constexpr size_t operator()(const ResourceId& id) const noexcept {
    return hash<uint64_t>{}(static_cast<uint64_t>(id));
  }
};
}  // namespace std

// etl::hash 特化，使 ResourceId 可以作为 etl::unordered_map 的键

namespace etl {
template <>
struct hash<ResourceId> {
  size_t operator()(const ResourceId& id) const {
    return etl::hash<uint64_t>{}(static_cast<uint64_t>(id));
  }
};
}  // namespace etl

#endif /* SIMPLEKERNEL_SRC_TASK_INCLUDE_RESOURCE_ID_HPP_ */
