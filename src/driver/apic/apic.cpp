/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 管理器实现
 */

#include "apic.h"

#include "kernel_log.hpp"
#include "singleton.hpp"

/**
 * @brief APIC 管理器 - 提供统一的 APIC 系统接口
 */
class Apic {
 public:
  /// @name 构造/析构函数
  /// @{
  Apic() = default;
  Apic(const Apic&) = delete;
  Apic(Apic&&) = delete;
  auto operator=(const Apic&) -> Apic& = delete;
  auto operator=(Apic&&) -> Apic& = delete;
  ~Apic() = default;
  /// @}

  /**
   * @brief 初始化 APIC 系统
   * @return true 初始化成功
   * @return false 初始化失败
   */
  auto Init() -> bool {
    klog::Info("初始化 APIC 系统...\n");

    // 检查 x2APIC 支持
    if (!LocalApic::IsX2ApicSupported()) {
      klog::Err("系统不支持 x2APIC，无法初始化 APIC\n");
      return false;
    }

    // 初始化 Local APIC
    if (!local_apic_.Init()) {
      klog::Err("Local APIC 初始化失败\n");
      return false;
    }

    initialized_ = true;

    klog::Info("APIC 系统初始化成功\n");
    klog::Info("  - Local APIC ID: 0x%x\n", GetCurrentApicId());
    klog::Info("  - APIC Version: 0x%x\n", GetApicVersion());

    return true;
  }

  /**
   * @brief 检查 APIC 是否已初始化
   * @return true 已初始化
   * @return false 未初始化
   */
  auto IsInitialized() const -> bool { return initialized_; }

  /**
   * @brief 获取当前处理器的 APIC ID
   * @return uint32_t APIC ID
   */
  auto GetCurrentApicId() const -> uint32_t { return LocalApic::GetApicId(); }

  /**
   * @brief 获取 APIC 版本
   * @return uint32_t APIC 版本
   */
  auto GetApicVersion() const -> uint32_t { return LocalApic::GetVersion(); }

  /**
   * @brief 发送中断结束信号
   */
  void SendEoi() const { LocalApic::SendEoi(); }

  /**
   * @brief 启动系统定时器
   * @param frequency_hz 定时器频率 (Hz)
   * @param vector 中断向量号
   * @return true 启动成功
   * @return false 启动失败
   */
  auto StartSystemTimer(uint32_t frequency_hz, uint32_t vector) -> bool {
    if (!initialized_) {
      klog::Err("APIC 未初始化，无法启动定时器\n");
      return false;
    }

    // 假设 APIC 时钟频率为 1GHz (这通常需要通过测量获得)
    const uint32_t apic_freq = 1000000000;  // 1 GHz
    uint32_t initial_count = apic_freq / frequency_hz;

    LocalApic::StartTimer(initial_count, vector, true, 16);

    klog::Info("系统定时器已启动 - 频率: %u Hz, 向量: %u\n", frequency_hz,
               vector);
    return true;
  }

  /**
   * @brief 停止系统定时器
   */
  void StopSystemTimer() const {
    LocalApic::StopTimer();
    klog::Info("系统定时器已停止\n");
  }

  /**
   * @brief 发送 IPI 到指定处理器
   * @param target_apic_id 目标处理器 APIC ID
   * @param vector 中断向量号
   */
  void SendIpi(uint32_t target_apic_id, uint32_t vector) const {
    if (!initialized_) {
      klog::Err("APIC 未初始化，无法发送 IPI\n");
      return;
    }

    LocalApic::SendIpi(target_apic_id, vector);
    klog::Info("已发送 IPI - 目标: 0x%x, 向量: %u\n", target_apic_id, vector);
  }

  /**
   * @brief 广播 IPI 到所有其他处理器
   * @param vector 中断向量号
   */
  void BroadcastIpi(uint32_t vector) const {
    if (!initialized_) {
      klog::Err("APIC 未初始化，无法广播 IPI\n");
      return;
    }

    LocalApic::BroadcastIpi(vector);
    klog::Info("已广播 IPI - 向量: %u\n", vector);
  }

  /**
   * @brief 设置任务优先级
   * @param priority 优先级值 (0-255)
   */
  void SetTaskPriority(uint32_t priority) const {
    LocalApic::SetTaskPriority(priority);
  }

  /**
   * @brief 获取任务优先级
   * @return uint32_t 当前任务优先级
   */
  auto GetTaskPriority() const -> uint32_t {
    return LocalApic::GetTaskPriority();
  }

 private:
  LocalApic local_apic_;
  bool initialized_ = false;
};

// 全局 APIC 管理器实例
using GlobalApicManager = Singleton<Apic>;

/**
 * @brief 初始化全局 APIC 系统
 * @return true 初始化成功
 * @return false 初始化失败
 */
auto InitApic() -> bool { return GlobalApicManager::GetInstance().Init(); }

/**
 * @brief 检查 APIC 是否已初始化
 * @return true 已初始化
 * @return false 未初始化
 */
auto IsApicInitialized() -> bool {
  return GlobalApicManager::GetInstance().IsInitialized();
}

/**
 * @brief 获取当前处理器的 APIC ID
 * @return uint32_t APIC ID
 */
auto GetCurrentApicId() -> uint32_t {
  return GlobalApicManager::GetInstance().GetCurrentApicId();
}

/**
 * @brief 发送中断结束信号
 */
void SendEoi() { GlobalApicManager::GetInstance().SendEoi(); }

/**
 * @brief 启动系统定时器
 * @param frequency_hz 定时器频率 (Hz)
 * @param vector 中断向量号
 * @return true 启动成功
 * @return false 启动失败
 */
auto StartSystemTimer(uint32_t frequency_hz, uint32_t vector) -> bool {
  return GlobalApicManager::GetInstance().StartSystemTimer(frequency_hz,
                                                           vector);
}

/**
 * @brief 停止系统定时器
 */
void StopSystemTimer() { GlobalApicManager::GetInstance().StopSystemTimer(); }

/**
 * @brief 发送 IPI 到指定处理器
 * @param target_apic_id 目标处理器 APIC ID
 * @param vector 中断向量号
 */
void SendIpi(uint32_t target_apic_id, uint32_t vector) {
  GlobalApicManager::GetInstance().SendIpi(target_apic_id, vector);
}

/**
 * @brief 广播 IPI 到所有其他处理器
 * @param vector 中断向量号
 */
void BroadcastIpi(uint32_t vector) {
  GlobalApicManager::GetInstance().BroadcastIpi(vector);
}
