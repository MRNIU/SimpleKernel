/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 驱动头文件
 */

#ifndef SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_IO_APIC_H_
#define SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_IO_APIC_H_

#include <cpu_io.h>

#include <array>
#include <cstdint>

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
  auto Init(uint64_t base_address) -> bool;

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
  void WriteRedirectionEntry(uint8_t irq, uint64_t value);
};

#endif /* SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_IO_APIC_H_ */
