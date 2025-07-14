
/**
 * @file 8259a.h
 * @brief 8259a 头文件
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

#ifndef SIMPLEKERNEL_SRC_KERNEL_DRIVER_8259A_INCLUDE_8259A_H_
#define SIMPLEKERNEL_SRC_KERNEL_DRIVER_8259A_INCLUDE_8259A_H_

#include <cstdint>

/**
 * @brief 8259A 中断控制器驱动
 */
class PIC8259A {
 public:
  /**
   * 构造函数
   * @param dev_addr 设备地址
   */
  explicit PIC8259A(uint64_t dev_addr);

  /// @name 默认构造/析构函数
  /// @{
  PIC8259A() = delete;
  PIC8259A(const PIC8259A& na16550a) = delete;
  PIC8259A(PIC8259A&& na16550a) = delete;
  auto operator=(const PIC8259A& na16550a) -> PIC8259A& = delete;
  auto operator=(PIC8259A&& na16550a) -> PIC8259A& = delete;
  ~PIC8259A() = default;
  /// @}

 private:
  uint64_t base_addr_ = 0;
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_DRIVER_8259A_INCLUDE_8259A_H_ */
