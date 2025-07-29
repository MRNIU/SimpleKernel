/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 头文件
 */

#ifndef SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_APIC_H_
#define SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_APIC_H_

#include <cstdint>

/**
 * @brief Local APIC 类 - 仅支持 x2APIC 模式
 */
class LocalApic {
 public:
  /// @name 构造/析构函数
  /// @{
  LocalApic() = default;
  LocalApic(const LocalApic&) = delete;
  LocalApic(LocalApic&&) = delete;
  auto operator=(const LocalApic&) -> LocalApic& = delete;
  auto operator=(LocalApic&&) -> LocalApic& = delete;
  ~LocalApic() = default;
  /// @}

  /**
   * @brief 初始化 Local APIC（x2APIC 模式）
   * @return true 初始化成功
   * @return false 初始化失败（不支持 x2APIC）
   */
  auto Init() -> bool;

  /**
   * @brief 检查是否支持 x2APIC
   * @return true 支持 x2APIC
   * @return false 不支持 x2APIC
   */
  static auto IsX2ApicSupported() -> bool;

  /**
   * @brief 检查 x2APIC 是否已启用
   * @return true x2APIC 已启用
   * @return false x2APIC 未启用
   */
  static auto IsX2ApicEnabled() -> bool;

  /**
   * @brief 启用 x2APIC 模式
   */
  static void EnableX2Apic();

  /**
   * @brief 获取当前处理器的 APIC ID
   * @return uint32_t APIC ID
   */
  static auto GetApicId() -> uint32_t;

  /**
   * @brief 获取 APIC 版本信息
   * @return uint32_t 版本信息
   */
  static auto GetVersion() -> uint32_t;

  /**
   * @brief 发送中断结束信号 (EOI)
   */
  static void SendEoi();

  /**
   * @brief 设置任务优先级
   * @param priority 优先级 (0-255)
   */
  static void SetTaskPriority(uint32_t priority);

  /**
   * @brief 获取任务优先级
   * @return uint32_t 当前任务优先级
   */
  static auto GetTaskPriority() -> uint32_t;

  /**
   * @brief 发送处理器间中断 (IPI)
   * @param destination_id 目标处理器 APIC ID
   * @param vector 中断向量号
   * @param delivery_mode 传递模式
   */
  static void SendIpi(uint32_t destination_id, uint32_t vector,
                      uint32_t delivery_mode = 0);

  /**
   * @brief 广播 IPI 到所有处理器（除自己外）
   * @param vector 中断向量号
   * @param delivery_mode 传递模式
   */
  static void BroadcastIpi(uint32_t vector, uint32_t delivery_mode = 0);

  /**
   * @brief 启动定时器
   * @param initial_count 初始计数值
   * @param vector 定时器中断向量号
   * @param periodic 是否为周期模式
   * @param divide_value 分频值 (1, 2, 4, 8, 16, 32, 64, 128)
   */
  static void StartTimer(uint32_t initial_count, uint32_t vector,
                         bool periodic = false, uint32_t divide_value = 1);

  /**
   * @brief 停止定时器
   */
  static void StopTimer();

  /**
   * @brief 获取定时器当前计数
   * @return uint32_t 当前计数值
   */
  static auto GetTimerCurrentCount() -> uint32_t;

  /**
   * @brief 设置虚假中断向量寄存器 (SIVR)
   * @param vector 虚假中断向量号
   * @param enable 是否启用 APIC
   */
  static void SetSpuriousVector(uint32_t vector, bool enable = true);

  /**
   * @brief 配置 LVT 表项
   * @param lvt_register LVT 寄存器 MSR 地址
   * @param vector 中断向量号
   * @param delivery_mode 传递模式
   * @param masked 是否屏蔽
   */
  static void ConfigureLvt(uint32_t lvt_register, uint32_t vector,
                           uint32_t delivery_mode = 0, bool masked = false);

 private:
  /// x2APIC 传递模式
  enum DeliveryMode {
    kFixed = 0x0,
    kLowestPriority = 0x1,
    kSmi = 0x2,
    kNmi = 0x4,
    kInit = 0x5,
    kStartup = 0x6
  };

  /// x2APIC 定时器模式
  enum TimerMode { kOneShot = 0x0, kPeriodic = 0x1, kTscDeadline = 0x2 };

  /// x2APIC 分频值映射
  static auto GetDivideConfig(uint32_t divide_value) -> uint32_t;
};

/**
 * @brief 初始化全局 APIC 系统
 * @return true 初始化成功
 * @return false 初始化失败
 */
auto InitApic() -> bool;

/**
 * @brief 检查 APIC 是否已初始化
 * @return true 已初始化
 * @return false 未初始化
 */
auto IsApicInitialized() -> bool;

/**
 * @brief 获取当前处理器的 APIC ID
 * @return uint32_t APIC ID
 */
auto GetCurrentApicId() -> uint32_t;

/**
 * @brief 发送中断结束信号
 */
void SendEoi();

/**
 * @brief 启动系统定时器
 * @param frequency_hz 定时器频率 (Hz)
 * @param vector 中断向量号
 * @return true 启动成功
 * @return false 启动失败
 */
auto StartSystemTimer(uint32_t frequency_hz, uint32_t vector) -> bool;

/**
 * @brief 停止系统定时器
 */
void StopSystemTimer();

/**
 * @brief 发送 IPI 到指定处理器
 * @param target_apic_id 目标处理器 APIC ID
 * @param vector 中断向量号
 */
void SendIpi(uint32_t target_apic_id, uint32_t vector);

/**
 * @brief 广播 IPI 到所有其他处理器
 * @param vector 中断向量号
 */
void BroadcastIpi(uint32_t vector);

#endif /* SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_APIC_H_ */
