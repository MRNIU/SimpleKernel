/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_IO_APIC_H_
#define SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_IO_APIC_H_

#include <cpu_io.h>

#include <array>
#include <cstdint>

/**
 * @brief IO APIC 驱动类
 * @note IO APIC 是系统级别的中断控制器，负责处理外部设备中断
 */
class IoApic {
 public:
  IoApic();

  /// @name 默认构造/析构函数
  /// @{
  IoApic(const IoApic&) = delete;
  IoApic(IoApic&&) = default;
  auto operator=(const IoApic&) -> IoApic& = delete;
  auto operator=(IoApic&&) -> IoApic& = default;
  ~IoApic() = default;
  /// @}

  /**
   * @brief 设置 IO APIC 重定向表项
   * @param irq IRQ 号
   * @param vector 中断向量
   * @param destination_apic_id 目标 APIC ID
   * @param mask 是否屏蔽中断
   */
  void SetIrqRedirection(uint8_t irq, uint8_t vector,
                         uint32_t destination_apic_id, bool mask = false) const;

  /**
   * @brief 屏蔽 IRQ
   * @param irq IRQ 号
   */
  void MaskIrq(uint8_t irq) const;

  /**
   * @brief 取消屏蔽 IRQ
   * @param irq IRQ 号
   */
  void UnmaskIrq(uint8_t irq) const;

  /**
   * @brief 获取 IO APIC ID
   * @return uint32_t IO APIC ID
   */
  [[nodiscard]] auto GetId() const -> uint32_t;

  /**
   * @brief 获取 IO APIC 版本
   * @return uint32_t IO APIC 版本
   */
  [[nodiscard]] auto GetVersion() const -> uint32_t;

  /**
   * @brief 获取 IO APIC 最大重定向条目数
   * @return uint32_t 最大重定向条目数
   */
  [[nodiscard]] auto GetMaxRedirectionEntries() const -> uint32_t;

  /**
   * @brief 打印 IO APIC 信息（调试用）
   */
  void PrintInfo() const;

 private:
  /// @name IO APIC 寄存器偏移常数
  /// @{
  /// 寄存器选择偏移
  static constexpr uint32_t kRegSel = 0x00;
  /// 寄存器窗口偏移
  static constexpr uint32_t kRegWin = 0x10;
  /// @}

  /// @name IO APIC 寄存器索引常数
  /// @{
  /// IO APIC ID 寄存器索引
  static constexpr uint32_t kRegId = 0x00;
  /// IO APIC 版本寄存器索引
  static constexpr uint32_t kRegVer = 0x01;
  /// IO APIC 仲裁寄存器索引
  static constexpr uint32_t kRegArb = 0x02;
  /// 重定向表基址索引
  static constexpr uint32_t kRedTblBase = 0x10;
  /// @}

  /// @name 重定向表项位字段常数
  /// @{
  /// 中断向量位掩码 (位 0-7)
  static constexpr uint64_t kVectorMask = 0xFF;
  /// 传递模式位移 (位 8-10)
  static constexpr uint32_t kDeliveryModeShift = 8;
  /// 目标模式位 (位 11)
  static constexpr uint64_t kDestModeBit = 1ULL << 11;
  /// 传递状态位 (位 12)
  static constexpr uint64_t kDeliveryStatusBit = 1ULL << 12;
  /// 极性位 (位 13)
  static constexpr uint64_t kPolarityBit = 1ULL << 13;
  /// 远程 IRR 位 (位 14)
  static constexpr uint64_t kRemoteIrrBit = 1ULL << 14;
  /// 触发模式位 (位 15)
  static constexpr uint64_t kTriggerModeBit = 1ULL << 15;
  /// 屏蔽位 (位 16)
  static constexpr uint64_t kMaskBit = 1ULL << 16;
  /// 目标 APIC ID 位移 (位 56-63)
  static constexpr uint32_t kDestApicIdShift = 56;
  /// 目标 APIC ID 掩码
  static constexpr uint64_t kDestApicIdMask = 0xFF;
  /// @}

  /// @name 传递模式常数
  /// @{
  /// 固定传递模式
  static constexpr uint32_t kDeliveryModeFixed = 0x0;
  /// 最低优先级传递模式
  static constexpr uint32_t kDeliveryModeLowestPriority = 0x1;
  /// SMI 传递模式
  static constexpr uint32_t kDeliveryModeSmi = 0x2;
  /// NMI 传递模式
  static constexpr uint32_t kDeliveryModeNmi = 0x4;
  /// INIT 传递模式
  static constexpr uint32_t kDeliveryModeInit = 0x5;
  /// ExtINT 传递模式
  static constexpr uint32_t kDeliveryModeExtInt = 0x7;
  /// @}

  /// @name IO APIC 基地址相关常数
  /// @{
  /// 默认 IO APIC 基地址
  static constexpr uint64_t kDefaultIoApicBase = 0xFEC00000;
  /// @}

  /// @brief IO APIC 基地址
  uint64_t base_address_ = kDefaultIoApicBase;

  /**
   * @brief 读取 IO APIC 寄存器
   * @param reg 寄存器索引
   * @return uint32_t 寄存器值
   */
  [[nodiscard]] auto Read(uint32_t reg) const -> uint32_t;

  /**
   * @brief 写入 IO APIC 寄存器
   * @param reg 寄存器索引
   * @param value 要写入的值
   */
  void Write(uint32_t reg, uint32_t value) const;

  /**
   * @brief 读取 IO APIC 重定向表项
   * @param irq IRQ 号
   * @return uint64_t 重定向表项值
   */
  [[nodiscard]] auto ReadRedirectionEntry(uint8_t irq) const -> uint64_t;

  /**
   * @brief 写入 IO APIC 重定向表项
   * @param irq IRQ 号
   * @param value 重定向表项值
   */
  void WriteRedirectionEntry(uint8_t irq, uint64_t value) const;
};

#endif /* SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_IO_APIC_H_ */
