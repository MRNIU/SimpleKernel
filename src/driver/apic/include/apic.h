/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 驱动头文件
 */

#ifndef SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_APIC_H_
#define SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_APIC_H_

#include <cpu_io.h>

#include <array>
#include <cstdint>

#include "io_apic.h"
#include "local_apic.h"

/**
 * @brief APIC 管理类，管理整个系统的 Local APIC 和 IO APIC
 * @note 在多核系统中：
 *       - Local APIC 是 per-CPU 的，每个核心通过 MSR 访问自己的 Local APIC
 *       - IO APIC 是系统级别的，通常有 1-2 个，处理外部中断
 */
class Apic {
 public:
  explicit Apic(const size_t cpu_count);

  /// @name 默认构造/析构函数
  /// @{
  Apic() = default;
  Apic(const Apic&) = delete;
  Apic(Apic&&) = default;
  auto operator=(const Apic&) -> Apic& = delete;
  auto operator=(Apic&&) -> Apic& = default;
  ~Apic() = default;
  /// @}

  /**
   * @brief 初始化当前 CPU 的 Local APIC
   * @return true 初始化成功
   * @return false 初始化失败
   * @note 每个 CPU 核心启动时都需要调用此函数
   */
  auto InitCurrentCpuLocalApic() -> bool;

  /**
   * @brief 设置 IRQ 重定向
   * @param irq IRQ 号
   * @param vector 中断向量
   * @param destination_apic_id 目标 APIC ID
   * @param mask 是否屏蔽中断
   * @return true 设置成功
   * @return false 设置失败
   */
  auto SetIrqRedirection(uint8_t irq, uint8_t vector,
                         uint32_t destination_apic_id,
                         bool mask = false) -> bool;

  /**
   * @brief 屏蔽 IRQ
   * @param irq IRQ 号
   * @return true 操作成功
   * @return false 操作失败
   */
  auto MaskIrq(uint8_t irq) -> bool;

  /**
   * @brief 取消屏蔽 IRQ
   * @param irq IRQ 号
   * @return true 操作成功
   * @return false 操作失败
   */
  auto UnmaskIrq(uint8_t irq) -> bool;

  /**
   * @brief 发送 IPI 到指定 CPU
   * @param target_apic_id 目标 CPU 的 APIC ID
   * @param vector 中断向量
   */
  void SendIpi(uint32_t target_apic_id, uint8_t vector) const;

  /**
   * @brief 广播 IPI 到所有其他 CPU
   * @param vector 中断向量
   */
  void BroadcastIpi(uint8_t vector) const;

  /**
   * @brief 启动 AP (Application Processor)
   * @param apic_id 目标 APIC ID
   * @param ap_code_addr AP 启动代码的虚拟地址
   * @param ap_code_size AP 启动代码的大小
   * @param target_addr AP 代码要复制到的目标物理地址
   * @return true 启动成功
   * @return false 启动失败
   * @note 函数内部会将启动代码复制到指定的目标地址，并计算 start_vector
   */
  auto StartupAp(uint32_t apic_id, uint64_t ap_code_addr, size_t ap_code_size,
                 uint64_t target_addr) const -> bool;

  /**
   * @brief 唤醒所有应用处理器 (AP)
   * @param ap_code_addr AP 启动代码的虚拟地址
   * @param ap_code_size AP 启动代码的大小
   * @param target_addr AP 代码要复制到的目标物理地址
   * @note 此方法会尝试唤醒除当前 BSP 外的所有 CPU 核心
   * @note 函数内部会将启动代码复制到指定的目标地址，并计算 start_vector
   */
  void StartupAllAps(uint64_t ap_code_addr, size_t ap_code_size,
                     uint64_t target_addr) const;

  /**
   * @brief 发送 EOI 信号给当前 CPU 的 Local APIC
   */
  void SendEoi() const;

  /**
   * @brief 设置 Local APIC 定时器
   * @param frequency_hz 定时器频率（Hz）
   * @param vector 中断向量号
   */
  void SetupPeriodicTimer(uint32_t frequency_hz, uint8_t vector) const;

  /**
   * @brief 打印所有 APIC 信息（调试用）
   */
  void PrintInfo() const;

 private:
  /// Local APIC 操作接口（静态实例，用于当前 CPU）
  LocalApic local_apic_;

  /// 只支持一个 IO APIC
  IoApic io_apic_;

  /// 系统 CPU 数量
  size_t cpu_count_;
};

#endif /* SIMPLEKERNEL_SRC_DRIVER_APIC_INCLUDE_APIC_H_ */
