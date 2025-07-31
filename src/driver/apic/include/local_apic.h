/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 驱动头文件
 */

#ifndef SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_LOCAL_APIC_H_
#define SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_LOCAL_APIC_H_

#include <cpu_io.h>

#include <array>
#include <cstdint>

/**
 * @brief Local APIC 驱动类
 * @note 支持 xAPIC 和 x2APIC 模式，优先使用 x2APIC
 */
class LocalApic {
 public:
  /// @name 默认构造/析构函数
  /// @{
  LocalApic() = default;
  LocalApic(const LocalApic&) = delete;
  LocalApic(LocalApic&&) = default;
  auto operator=(const LocalApic&) -> LocalApic& = delete;
  auto operator=(LocalApic&&) -> LocalApic& = default;
  ~LocalApic() = default;
  /// @}

  /**
   * @brief 初始化 Local APIC
   * @return true 初始化成功
   * @return false 初始化失败
   */
  auto Init() -> bool;

  /**
   * @brief 启用 x2APIC 模式
   * @return true 启用成功
   * @return false 启用失败
   */
  auto EnableX2Apic() -> bool;

  /**
   * @brief 禁用 x2APIC 模式
   */
  void DisableX2Apic();

  /**
   * @brief 检查 x2APIC 是否启用
   * @return true x2APIC 已启用
   * @return false x2APIC 未启用
   */
  [[nodiscard]] auto IsX2ApicEnabled() const -> bool;

  /**
   * @brief 获取当前 CPU 的 APIC ID
   * @return uint32_t APIC ID
   */
  [[nodiscard]] auto GetApicId() const -> uint32_t;

  /**
   * @brief 获取 APIC 版本信息
   * @return uint32_t APIC 版本
   */
  [[nodiscard]] auto GetApicVersion() const -> uint32_t;

  /**
   * @brief 发送中断结束信号 (EOI)
   */
  void SendEoi();

  /**
   * @brief 发送处理器间中断 (IPI)
   * @param destination_apic_id 目标 APIC ID
   * @param vector 中断向量
   */
  void SendIpi(uint32_t destination_apic_id, uint8_t vector);

  /**
   * @brief 广播 IPI 到所有其他 CPU
   * @param vector 中断向量
   */
  void BroadcastIpi(uint8_t vector);

  /**
   * @brief 设置任务优先级
   * @param priority 优先级值
   */
  void SetTaskPriority(uint8_t priority);

  /**
   * @brief 获取任务优先级
   * @return uint8_t 当前任务优先级
   */
  [[nodiscard]] auto GetTaskPriority() const -> uint8_t;

  /**
   * @brief 启用 Local APIC 定时器
   * @param initial_count 初始计数值
   * @param divide_value 分频值
   * @param vector 定时器中断向量
   * @param periodic 是否为周期性定时器
   */
  void EnableTimer(uint32_t initial_count, uint32_t divide_value,
                   uint8_t vector, bool periodic = true);

  /**
   * @brief 禁用 Local APIC 定时器
   */
  void DisableTimer();

  /**
   * @brief 获取定时器当前计数值
   * @return uint32_t 当前计数值
   */
  [[nodiscard]] auto GetTimerCurrentCount() const -> uint32_t;

  /**
   * @brief 设置周期性定时器
   * @param frequency_hz 定时器频率（Hz）
   * @param vector 定时器中断向量
   */
  void SetupPeriodicTimer(uint32_t frequency_hz, uint8_t vector);

  /**
   * @brief 设置单次定时器
   * @param microseconds 延时时间（微秒）
   * @param vector 定时器中断向量
   */
  void SetupOneShotTimer(uint32_t microseconds, uint8_t vector);

  /**
   * @brief 校准 APIC 定时器频率
   * @return uint32_t APIC 定时器频率（Hz）
   */
  auto CalibrateTimer() -> uint32_t;

  /**
   * @brief 发送 INIT IPI
   * @param destination_apic_id 目标 APIC ID
   */
  void SendInitIpi(uint32_t destination_apic_id);

  /**
   * @brief 发送 SIPI (Startup IPI)
   * @param destination_apic_id 目标 APIC ID
   * @param start_page 启动页面地址（4KB 页面）
   */
  void SendStartupIpi(uint32_t destination_apic_id, uint8_t start_page);

  /**
   * @brief 唤醒应用处理器 (AP)
   * @param destination_apic_id 目标 APIC ID
   * @param start_vector 启动向量（启动代码的物理地址 / 4096）
   * @return true 唤醒成功
   * @return false 唤醒失败
   * @note 执行标准的 INIT-SIPI-SIPI 序列来唤醒 AP
   */
  auto WakeupAp(uint32_t destination_apic_id, uint8_t start_vector) -> bool;

  /**
   * @brief 配置 Local Vector Table 条目
   */
  void ConfigureLvtEntries();

  /**
   * @brief 读取错误状态
   * @return uint32_t 错误状态寄存器值
   */
  auto ReadErrorStatus() -> uint32_t;

  /**
   * @brief 打印 Local APIC 信息（调试用）
   */
  void PrintInfo() const;

 private:
  /**
   * @brief 检查 CPU 是否支持 x2APIC
   * @return true 支持 x2APIC
   * @return false 不支持 x2APIC
   */
  [[nodiscard]] auto CheckX2ApicSupport() const -> bool;

  /**
   * @brief 启用传统 xAPIC 模式
   * @return true 启用成功
   * @return false 启用失败
   */
  auto EnableXApic() -> bool;

  /**
   * @brief 禁用传统 xAPIC 模式
   */
  void DisableXApic();

  /**
   * @brief 检查传统 xAPIC 是否启用
   * @return true xAPIC 已启用
   * @return false xAPIC 未启用
   */
  [[nodiscard]] auto IsXApicEnabled() const -> bool;

  /**
   * @brief 获取 APIC 基地址
   * @return uint64_t APIC 基地址
   */
  [[nodiscard]] auto GetApicBase() const -> uint64_t;

  /**
   * @brief 设置 APIC 基地址
   * @param base_address APIC 基地址
   */
  void SetApicBase(uint64_t base_address);

  /// @name xAPIC 寄存器偏移量常数
  /// @{
  /// APIC ID 寄存器偏移
  static constexpr uint32_t kXApicIdOffset = 0x20;
  /// 版本寄存器偏移
  static constexpr uint32_t kXApicVersionOffset = 0x30;
  /// 任务优先级寄存器偏移
  static constexpr uint32_t kXApicTprOffset = 0x80;
  /// EOI 寄存器偏移
  static constexpr uint32_t kXApicEoiOffset = 0xB0;
  /// 虚假中断向量寄存器偏移
  static constexpr uint32_t kXApicSivrOffset = 0xF0;
  /// 错误状态寄存器偏移
  static constexpr uint32_t kXApicEsrOffset = 0x280;
  /// ICR 低位寄存器偏移
  static constexpr uint32_t kXApicIcrLowOffset = 0x300;
  /// ICR 高位寄存器偏移
  static constexpr uint32_t kXApicIcrHighOffset = 0x310;
  /// LVT 定时器寄存器偏移
  static constexpr uint32_t kXApicLvtTimerOffset = 0x320;
  /// LVT LINT0 寄存器偏移
  static constexpr uint32_t kXApicLvtLint0Offset = 0x350;
  /// LVT LINT1 寄存器偏移
  static constexpr uint32_t kXApicLvtLint1Offset = 0x360;
  /// LVT 错误寄存器偏移
  static constexpr uint32_t kXApicLvtErrorOffset = 0x370;
  /// 定时器初始计数寄存器偏移
  static constexpr uint32_t kXApicTimerInitCountOffset = 0x380;
  /// 定时器当前计数寄存器偏移
  static constexpr uint32_t kXApicTimerCurrCountOffset = 0x390;
  /// 定时器分频寄存器偏移
  static constexpr uint32_t kXApicTimerDivideOffset = 0x3E0;
  /// @}

  /// @name 位掩码和位移常数
  /// @{
  /// xAPIC ID 位移
  static constexpr uint32_t kApicIdShift = 24;
  /// xAPIC ID 掩码
  static constexpr uint32_t kApicIdMask = 0xFF;
  /// APIC 软件启用位
  static constexpr uint32_t kApicSoftwareEnableBit = 0x100;
  /// 虚假中断向量
  static constexpr uint32_t kSpuriousVector = 0xFF;
  /// LVT 掩码位
  static constexpr uint32_t kLvtMaskBit = 0x10000;
  /// LVT 周期模式位
  static constexpr uint32_t kLvtPeriodicMode = 0x20000;
  /// ICR 传递状态位
  static constexpr uint32_t kIcrDeliveryStatusBit = 0x1000;
  /// ICR 目标位移
  static constexpr uint32_t kIcrDestShift = 24;
  /// ICR 广播模式位
  static constexpr uint32_t kIcrBroadcastMode = 0xC0000;
  /// INIT IPI 模式
  static constexpr uint32_t kInitIpiMode = 0x500;
  /// SIPI 模式
  static constexpr uint32_t kSipiMode = 0x600;
  /// ExtINT 传递模式
  static constexpr uint32_t kExtIntMode = 0x700;
  /// NMI 传递模式
  static constexpr uint32_t kNmiMode = 0x400;
  /// 错误中断向量
  static constexpr uint8_t kErrorVector = 0xEF;
  /// @}

  /// @name 定时器相关常数
  /// @{
  /// 默认 APIC 时钟频率 (100MHz)
  static constexpr uint32_t kDefaultApicClockHz = 100000000;
  /// 定时器分频 1
  static constexpr uint32_t kTimerDivideBy1 = 0x0B;
  /// 定时器分频 16
  static constexpr uint32_t kTimerDivideBy16 = 0x03;
  /// 校准用的计数值
  static constexpr uint32_t kCalibrationCount = 0xFFFFFFFF;
  /// 校准延时循环次数
  static constexpr uint32_t kCalibrationDelayLoop = 1000000;
  /// 校准倍数 (10ms -> 1s)
  static constexpr uint32_t kCalibrationMultiplier = 100;
  /// 每秒微秒数
  static constexpr uint32_t kMicrosecondsPerSecond = 1000000;
  /// @}

  /// @name APIC 基地址相关常数
  /// @{
  /// 默认 APIC 基地址
  static constexpr uint64_t kDefaultApicBase = 0xFEE00000;
  /// APIC 基地址掩码
  static constexpr uint64_t kApicBaseMask = 0xFFFFF000ULL;
  /// APIC 全局启用位
  static constexpr uint64_t kApicGlobalEnableBit = 1ULL << 11;
  /// x2APIC 启用位
  static constexpr uint64_t kX2ApicEnableBit = 1ULL << 10;
  /// APIC 基地址控制位掩码
  static constexpr uint64_t kApicBaseControlMask = 0xFFF;
  /// @}

  /// @brief 当前 APIC 模式（true = x2APIC, false = xAPIC）
  bool is_x2apic_mode_ = false;

  /// @brief APIC 基地址（仅用于 xAPIC 模式）
  uint64_t apic_base_ = kDefaultApicBase;
};

#endif /* SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_LOCAL_APIC_H_ */
