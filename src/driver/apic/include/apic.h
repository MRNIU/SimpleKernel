
/**
 * @file apic.h
 * @brief apic 头文件
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2024-05-24
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2024-05-24<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_DRIVER_APIC_INCLUDE_APIC_H_
#define SIMPLEKERNEL_SRC_KERNEL_DRIVER_APIC_INCLUDE_APIC_H_

#include <cstdint>

/**
 * @brief APIC 高级中断控制器驱动
 */
class Apic {
 public:
  /**
   * 构造函数
   * @param dev_addr 设备地址
   */
  explicit Apic(uint64_t dev_addr);

  /// @name 默认构造/析构函数
  /// @{
  Apic() = delete;
  Apic(const Apic& na16550a) = delete;
  Apic(Apic&& na16550a) = delete;
  auto operator=(const Apic& na16550a) -> Apic& = delete;
  auto operator=(Apic&& na16550a) -> Apic& = delete;
  ~Apic() = default;
  /// @}

 private:
  uint64_t base_addr_ = 0;
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_DRIVER_APIC_INCLUDE_APIC_H_ */
