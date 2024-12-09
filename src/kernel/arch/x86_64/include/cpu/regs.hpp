
/**
 * @file regs.hpp
 * @brief x86_64 寄存器相关定义
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2024-03-05
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2024-03-05<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INCLUDE_CPU_REGS_HPP_
#define SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INCLUDE_CPU_REGS_HPP_

#include <sys/cdefs.h>

#include <cstdint>
#include <cstdlib>
#include <type_traits>

#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"

/**
 * x86_64 cpu Control Registers 相关定义
 * @note 寄存器读写设计见 arch/README.md
 */
namespace cpu {

// 第一部分：寄存器定义
namespace register_info {

struct RegInfoBase {
  /// 寄存器数据类型
  using DataType = uint64_t;
  /// 起始位
  static constexpr uint64_t kBitOffset = 0;
  /// 位宽
  static constexpr uint64_t kBitWidth = 64;
  /// 掩码，(val & kBitMask) == 对应当前位的值
  static constexpr uint64_t kBitMask =
      (kBitWidth < 64) ? ((1ULL << kBitWidth) - 1) << kBitOffset : ~0ULL;
  /// 对应位置位掩码
  static constexpr uint64_t kAllSetMask =
      (kBitWidth < 64) ? ((1ULL << kBitWidth) - 1) : ~0ULL;
};

/// 通用寄存器
struct RbpInfo : public RegInfoBase {};

};  // namespace register_info

// 第二部分：读/写模版实现
namespace read_write {
/**
 * 只读接口
 * @tparam 寄存器类型
 */
template <class RegInfo>
class ReadOnlyRegBase {
 public:
  /// @name 构造/析构函数
  /// @{
  ReadOnlyRegBase() = default;
  ReadOnlyRegBase(const ReadOnlyRegBase &) = delete;
  ReadOnlyRegBase(ReadOnlyRegBase &&) = delete;
  auto operator=(const ReadOnlyRegBase &) -> ReadOnlyRegBase & = delete;
  auto operator=(ReadOnlyRegBase &&) -> ReadOnlyRegBase & = delete;
  ~ReadOnlyRegBase() = default;
  /// @}

  /**
   * 读寄存器
   * @return RegInfo::DataType 寄存器的值
   */
  static __always_inline auto Read() -> typename RegInfo::DataType {
    typename RegInfo::DataType value{};
    if constexpr (std::is_same_v<RegInfo, register_info::RbpInfo>) {
      __asm__ volatile("mov %%rbp, %0" : "=r"(value) : :);
    } else {
      klog::Err("No Type\n");
      throw;
    }
    return value;
  }

  /**
   * () 重载
   */
  __always_inline auto operator()() -> typename RegInfo::DataType {
    return Read();
  }
};

/**
 * 只写接口
 * @tparam 寄存器类型
 */
template <class RegInfo>
class WriteOnlyRegBase {
 public:
  /// @name 构造/析构函数
  /// @{
  WriteOnlyRegBase() = default;
  WriteOnlyRegBase(const WriteOnlyRegBase &) = delete;
  WriteOnlyRegBase(WriteOnlyRegBase &&) = delete;
  auto operator=(const WriteOnlyRegBase &) -> WriteOnlyRegBase & = delete;
  auto operator=(WriteOnlyRegBase &&) -> WriteOnlyRegBase & = delete;
  ~WriteOnlyRegBase() = default;
  /// @}

  /**
   * 写寄存器
   * @param value 要写的值
   */
  static __always_inline void Write(typename RegInfo::DataType value) {
    if constexpr (std::is_same_v<RegInfo, register_info::RbpInfo>) {
      __asm__ volatile("mv fp, %0" : : "r"(value) :);
    } else {
      klog::Err("No Type\n");
      throw;
    }
  }
};

/**
 * 读写接口
 * @tparam 寄存器类型
 */
template <class RegInfo>
class ReadWriteRegBase : public ReadOnlyRegBase<RegInfo>,
                         public WriteOnlyRegBase<RegInfo> {
 public:
  /// @name 构造/析构函数
  /// @{
  ReadWriteRegBase() = default;
  ReadWriteRegBase(const ReadWriteRegBase &) = delete;
  ReadWriteRegBase(ReadWriteRegBase &&) = delete;
  auto operator=(const ReadWriteRegBase &) -> ReadWriteRegBase & = delete;
  auto operator=(ReadWriteRegBase &&) -> ReadWriteRegBase & = delete;
  ~ReadWriteRegBase() = default;
  /// @}
};

};  // namespace read_write

namespace regs {

// 第三部分：寄存器实例
class Rbp : public read_write::ReadWriteRegBase<register_info::RbpInfo> {
 public:
  friend auto operator<<(sk_std::ostream &ostream,
                         [[maybe_unused]] const Rbp &rbp) -> sk_std::ostream & {
    klog::Info("val: 0x%p", regs::Rbp::Read());
    return ostream;
  }
};

/// 通用寄存器
struct AllXreg {
  Rbp rbp;
};

};  // namespace regs

// 第四部分：访问接口
[[maybe_unused]] static regs::AllXreg kAllXreg;

};  // namespace cpu

#endif  // SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INCLUDE_CPU_REGS_HPP_
