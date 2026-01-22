/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_RESOURCE_ID_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_RESOURCE_ID_HPP_

#include <cstdint>

/**
 * @brief 资源类型枚举
 */
enum class ResourceType : uint8_t {
  kNone = 0x00,        // 无效资源
  kMutex = 0x01,       // 互斥锁
  kSemaphore = 0x02,   // 信号量
  kCondVar = 0x03,     // 条件变量
  kChildExit = 0x04,   // 等待子进程退出
  kIoComplete = 0x05,  // IO 完成
  kFutex = 0x06,       // Futex (快速用户空间互斥锁)
  kSignal = 0x07,      // 信号
  kTimer = 0x08,       // 定时器
  kResourceTypeCount,
  // 可以继续扩展...
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

#endif /* SIMPLEKERNEL_SRC_TASK_INCLUDE_RESOURCE_ID_HPP_ */
