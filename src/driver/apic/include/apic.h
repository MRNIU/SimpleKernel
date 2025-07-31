/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 驱动头文件
 */

#ifndef SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_APIC_H_
#define SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_APIC_H_

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
  bool Init();

  /**
   * @brief 启用 x2APIC 模式
   * @return true 启用成功
   * @return false 启用失败
   */
  bool EnableX2Apic();

  /**
   * @brief 禁用 x2APIC 模式
   */
  void DisableX2Apic();

  /**
   * @brief 检查 x2APIC 是否启用
   * @return true x2APIC 已启用
   * @return false x2APIC 未启用
   */
  bool IsX2ApicEnabled() const;

  /**
   * @brief 获取当前 CPU 的 APIC ID
   * @return uint32_t APIC ID
   */
  uint32_t GetApicId() const;

  /**
   * @brief 获取 APIC 版本信息
   * @return uint32_t APIC 版本
   */
  uint32_t GetApicVersion() const;

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
  uint8_t GetTaskPriority() const;

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
  uint32_t GetTimerCurrentCount() const;

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
  uint32_t CalibrateTimer();

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
  bool WakeupAp(uint32_t destination_apic_id, uint8_t start_vector);

  /**
   * @brief 配置 Local Vector Table 条目
   */
  void ConfigureLvtEntries();

  /**
   * @brief 读取错误状态
   * @return uint32_t 错误状态寄存器值
   */
  uint32_t ReadErrorStatus();

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
  bool CheckX2ApicSupport() const;

  /**
   * @brief 启用传统 xAPIC 模式
   * @return true 启用成功
   * @return false 启用失败
   */
  bool EnableXApic();

  /**
   * @brief 禁用传统 xAPIC 模式
   */
  void DisableXApic();

  /**
   * @brief 检查传统 xAPIC 是否启用
   * @return true xAPIC 已启用
   * @return false xAPIC 未启用
   */
  bool IsXApicEnabled() const;

  /**
   * @brief 获取 APIC 基地址
   * @return uint64_t APIC 基地址
   */
  uint64_t GetApicBase() const;

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

/**
 * @brief IO APIC 驱动类
 */
class IoApic {
 public:
  /// @name 默认构造/析构函数
  /// @{
  IoApic() = default;
  IoApic(const IoApic&) = delete;
  IoApic(IoApic&&) = default;
  auto operator=(const IoApic&) -> IoApic& = delete;
  auto operator=(IoApic&&) -> IoApic& = default;
  ~IoApic() = default;
  /// @}

  /**
   * @brief 初始化 IO APIC
   * @param base_address IO APIC 基地址
   * @return true 初始化成功
   * @return false 初始化失败
   */
  bool Init(uint64_t base_address);

  /**
   * @brief 设置 IO APIC 重定向表项
   * @param irq IRQ 号
   * @param vector 中断向量
   * @param destination_apic_id 目标 APIC ID
   * @param mask 是否屏蔽中断
   */
  void SetIrqRedirection(uint8_t irq, uint8_t vector,
                         uint32_t destination_apic_id, bool mask = false);

  /**
   * @brief 屏蔽 IRQ
   * @param irq IRQ 号
   */
  void MaskIrq(uint8_t irq);

  /**
   * @brief 取消屏蔽 IRQ
   * @param irq IRQ 号
   */
  void UnmaskIrq(uint8_t irq);

  /**
   * @brief 获取 IO APIC ID
   * @return uint32_t IO APIC ID
   */
  uint32_t GetId() const;

  /**
   * @brief 获取 IO APIC 版本
   * @return uint32_t IO APIC 版本
   */
  uint32_t GetVersion() const;

  /**
   * @brief 获取 IO APIC 最大重定向条目数
   * @return uint32_t 最大重定向条目数
   */
  uint32_t GetMaxRedirectionEntries() const;

  /**
   * @brief 获取 IO APIC 基地址
   * @return uint64_t IO APIC 基地址
   */
  uint64_t GetBaseAddress() const;

  /**
   * @brief 打印 IO APIC 信息（调试用）
   */
  void PrintInfo() const;

 private:
  /// IO APIC 基地址
  uint64_t base_address_{0};

  /// @name IO APIC 寄存器偏移
  /// @{
  /// 寄存器选择
  static constexpr uint32_t kRegSel = 0x00;
  /// 寄存器窗口
  static constexpr uint32_t kRegWin = 0x10;
  /// @}

  /// @name IO APIC 寄存器索引
  /// @{
  /// IO APIC ID
  static constexpr uint32_t kRegId = 0x00;
  /// IO APIC 版本
  static constexpr uint32_t kRegVer = 0x01;
  /// IO APIC 仲裁
  static constexpr uint32_t kRegArb = 0x02;
  /// 重定向表基址
  static constexpr uint32_t kRedTblBase = 0x10;
  /// @}

  /**
   * @brief 读取 IO APIC 寄存器
   * @param reg 寄存器索引
   * @return uint32_t 寄存器值
   */
  uint32_t ReadReg(uint32_t reg) const;

  /**
   * @brief 写入 IO APIC 寄存器
   * @param reg 寄存器索引
   * @param value 要写入的值
   */
  void WriteReg(uint32_t reg, uint32_t value);

  /**
   * @brief 读取 IO APIC 重定向表项
   * @param irq IRQ 号
   * @return uint64_t 重定向表项值
   */
  uint64_t ReadRedirectionEntry(uint8_t irq) const;

  /**
   * @brief 写入 IO APIC 重定向表项
   * @param irq IRQ 号
   * @param value 重定向表项值
   */
  void WriteRedirectionEntry(uint8_t irq, uint64_t value);
};

/**
 * @brief APIC 管理类，管理整个系统的 Local APIC 和 IO APIC
 * @note 在多核系统中：
 *       - Local APIC 是 per-CPU 的，每个核心通过 MSR 访问自己的 Local APIC
 *       - IO APIC 是系统级别的，通常有 1-2 个，处理外部中断
 */
class Apic {
 public:
  /// @name 默认构造/析构函数
  /// @{
  Apic() = default;
  Apic(const Apic&) = delete;
  Apic(Apic&&) = default;
  auto operator=(const Apic&) -> Apic& = delete;
  auto operator=(Apic&&) -> Apic& = default;
  ~Apic() = default;
  /// @}

  /// 启动 APs 的默认地址和大小
  static constexpr uint64_t kDefaultAPBase = 0x30000;
  static constexpr uint64_t kDefaultAPSize = 0x10000;

  /**
   * @brief 初始化 APIC 系统
   * @param max_cpu_count 系统最大 CPU 数量
   * @return true 初始化成功
   * @return false 初始化失败
   */
  bool Init(size_t max_cpu_count);

  /**
   * @brief 添加 IO APIC
   * @param base_address IO APIC 基地址
   * @param gsi_base 全局系统中断基址
   * @return true 添加成功
   * @return false 添加失败
   */
  bool AddIoApic(uint64_t base_address, uint32_t gsi_base = 0);

  /**
   * @brief 初始化当前 CPU 的 Local APIC
   * @return true 初始化成功
   * @return false 初始化失败
   * @note 每个 CPU 核心启动时都需要调用此函数
   */
  bool InitCurrentCpuLocalApic();

  /**
   * @brief 获取当前 CPU 的 Local APIC 操作接口
   * @return LocalApic& Local APIC 引用
   * @note 返回的是一个静态实例，用于访问当前 CPU 的 Local APIC
   */
  LocalApic& GetCurrentLocalApic();

  /**
   * @brief 获取 IO APIC 实例
   * @param index IO APIC 索引
   * @return IoApic* IO APIC 指针，如果索引无效则返回 nullptr
   */
  IoApic* GetIoApic(size_t index = 0);

  /**
   * @brief 根据 GSI 查找对应的 IO APIC
   * @param gsi 全局系统中断号
   * @return IoApic* 对应的 IO APIC 指针，如果未找到则返回 nullptr
   */
  IoApic* FindIoApicByGsi(uint32_t gsi);

  /**
   * @brief 获取 IO APIC 数量
   * @return size_t IO APIC 数量
   */
  size_t GetIoApicCount() const;

  /**
   * @brief 设置 IRQ 重定向
   * @param irq IRQ 号
   * @param vector 中断向量
   * @param destination_apic_id 目标 APIC ID
   * @param mask 是否屏蔽中断
   * @return true 设置成功
   * @return false 设置失败
   */
  bool SetIrqRedirection(uint8_t irq, uint8_t vector,
                         uint32_t destination_apic_id, bool mask = false);

  /**
   * @brief 屏蔽 IRQ
   * @param irq IRQ 号
   * @return true 操作成功
   * @return false 操作失败
   */
  bool MaskIrq(uint8_t irq);

  /**
   * @brief 取消屏蔽 IRQ
   * @param irq IRQ 号
   * @return true 操作成功
   * @return false 操作失败
   */
  bool UnmaskIrq(uint8_t irq);

  /**
   * @brief 发送 IPI 到指定 CPU
   * @param target_apic_id 目标 CPU 的 APIC ID
   * @param vector 中断向量
   */
  void SendIpi(uint32_t target_apic_id, uint8_t vector);

  /**
   * @brief 广播 IPI 到所有其他 CPU
   * @param vector 中断向量
   */
  void BroadcastIpi(uint8_t vector);

  /**
   * @brief 启动 AP (Application Processor)
   * @param apic_id 目标 APIC ID
   * @param ap_code_addr AP 启动代码的虚拟地址
   * @param ap_code_size AP 启动代码的大小
   * @return true 启动成功
   * @return false 启动失败
   * @note 函数内部会将启动代码复制到 kDefaultAPBase，并计算 start_vector
   */
  bool StartupAp(uint32_t apic_id, const void* ap_code_addr,
                 size_t ap_code_size);

  /**
   * @brief 唤醒所有应用处理器 (AP)
   * @param ap_code_addr AP 启动代码的虚拟地址
   * @param ap_code_size AP 启动代码的大小
   * @param max_wait_ms 最大等待时间（毫秒）
   * @return size_t 成功启动的 AP 数量
   * @note 此方法会尝试唤醒除当前 BSP 外的所有 CPU 核心
   * @note 函数内部会将启动代码复制到 kDefaultAPBase，并计算 start_vector
   */
  size_t StartupAllAps(const void* ap_code_addr, size_t ap_code_size,
                       uint32_t max_wait_ms = 5000);

  /**
   * @brief 获取当前 CPU 的 APIC ID
   * @return uint32_t 当前 CPU 的 APIC ID
   */
  uint32_t GetCurrentApicId();

  /**
   * @brief 打印所有 APIC 信息（调试用）
   */
  void PrintInfo() const;

 private:
  /// 最大支持的 CPU 数量
  static constexpr size_t kMaxCpus = 256;
  /// 最大 IO APIC 数量
  static constexpr size_t kMaxIoApics = 8;

  /// IO APIC 信息结构
  struct IoApicInfo {
    IoApic instance;
    uint64_t base_address;
    uint32_t gsi_base;
    uint32_t gsi_count;
    bool valid;
  };

  /// Local APIC 操作接口（静态实例，用于当前 CPU）
  LocalApic local_apic_;

  /// IO APIC 实例数组
  std::array<IoApicInfo, kMaxIoApics> io_apics_;

  /// 当前 IO APIC 数量
  size_t io_apic_count_{0};

  /// 系统最大 CPU 数量
  size_t max_cpu_count_{2};

  /**
   * @brief 根据 IRQ 查找对应的 IO APIC
   * @param irq IRQ 号
   * @return IoApicInfo* 对应的 IO APIC 信息，如果未找到则返回 nullptr
   */
  IoApicInfo* FindIoApicByIrq(uint8_t irq);
};

#endif /* SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_APIC_H_ */
