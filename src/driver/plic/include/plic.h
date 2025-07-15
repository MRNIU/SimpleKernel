/**
 * @file plic.h
 * @brief plic 头文件
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

#ifndef SIMPLEKERNEL_SRC_DRIVER_PLIC_INCLUDE_PLIC_H_
#define SIMPLEKERNEL_SRC_DRIVER_PLIC_INCLUDE_PLIC_H_

#include <array>
#include <cstddef>
#include <cstdint>
#include <tuple>

/**
 * @brief Plic 驱动
 * @see https://github.com/riscv/riscv-plic
 */
class Plic {
 public:
  /**
   * @brief 中断处理函数指针
   * @param cause 中断号
   * @param context 中断上下文
   */
  typedef uint64_t (*InterruptFunc)(uint64_t cause, uint8_t* context);

  /**
   * 构造函数
   * @param dev_addr 设备地址
   * @param ndev 支持的中断源数量 (riscv,ndev)
   * @param context_count 上下文数量 (通常为 2 * core_count)
   */
  explicit Plic(uint64_t dev_addr, size_t ndev, size_t context_count);

  /// @name 默认构造/析构函数
  /// @{
  Plic() = default;
  Plic(const Plic& plic) = default;
  Plic(Plic&& plic) = default;
  auto operator=(const Plic& plic) -> Plic& = default;
  auto operator=(Plic&& plic) -> Plic& = default;
  ~Plic() = default;
  /// @}

  /**
   * @brief 执行中断处理
   * @param  cause 中断或异常号
   * @param  context 中断上下文
   */
  void Do(uint64_t cause, uint8_t* context) const;

  /**
   * @brief 向 Plic 询问中断
   * @return uint32_t 中断源 ID (1-1023)
   */
  auto Which() const -> uint32_t;

  /**
   * @brief 告知 Plic 已经处理了当前 IRQ
   * @param  source_id 中断源 ID (1-1023)
   */
  void Done(uint32_t source_id) const;

  /**
   * @brief 设置指定中断源的使能状态
   * @param hart_id hart ID
   * @param source_id 中断源 ID (1-1023)
   * @param priority 中断优先级 (0-7, 0 表示禁用)
   * @param enable 是否使能该中断
   */
  void Set(uint32_t hart_id, uint32_t source_id, uint32_t priority,
           bool enable) const;

  /**
   * @brief 获取指定中断源的状态信息
   * @param hart_id hart ID
   * @param source_id 中断源 ID (1-1023)
   * @return <优先级, 使能状态, 挂起状态>
   */
  auto Get(uint32_t hart_id, uint32_t source_id) const
      -> std::tuple<uint32_t, bool, bool>;

  /**
   * @brief 注册外部中断处理函数
   * @param  cause             外部中断号
   * @param  func 外部中断处理函数
   */
  void RegisterInterruptFunc(uint8_t cause, InterruptFunc func);

  /**
   * @brief 执行外部中断处理
   * @param  cause              外部中断号
   * @param  context 中断上下文
   */
  void Do(uint64_t cause, uint8_t* context);

 private:
  static constexpr const uint64_t kSourcePriorityOffset = 0x000000;
  static constexpr const uint64_t kPendingBitsOffset = 0x001000;
  static constexpr const uint64_t kEnableBitsOffset = 0x002000;
  static constexpr const uint64_t kContextOffset = 0x200000;

  // 每个 context 的大小和偏移
  static constexpr const uint64_t kContextSize = 0x1000;
  static constexpr const uint64_t kPriorityThresholdOffset = 0x0;
  static constexpr const uint64_t kClaimCompleteOffset = 0x4;

  // Enable bits 每个 context 的大小 (最多支持 1024 个中断源)
  static constexpr const uint64_t kEnableSize = 0x80;

  /// 最大外部中断数量
  static constexpr const size_t kInterruptMaxCount = 16;

  /// 外部中断处理函数数组
  static std::array<InterruptFunc, kInterruptMaxCount> interrupt_handlers_;

  uint64_t base_addr_ = 0;
  size_t ndev_ = 0;
  size_t context_count_ = 0;

  /**
   * @brief 计算 context ID
   * @param hart_id hart ID
   * @param mode 模式 (0=M-mode, 1=S-mode)
   * @return uint32_t context ID
   * @note 2 个模式的 context ID 计算方式为: hart_id * 2 + mode
   */
  __always_inline auto GetContextId(uint32_t hart_id, uint32_t mode = 1) const
      -> uint32_t {
    return hart_id * 2 + mode;
  }

  /**
   * @brief 获取使能位寄存器中指定中断源的状态
   * @param context_id context ID
   * @param source_id 中断源 ID (1-1023，0 保留)
   * @return bool 对应位的状态
   */
  auto GetEnableBit(uint32_t context_id, uint32_t source_id) const -> bool;

  /**
   * @brief 设置使能位寄存器中指定中断源的状态
   * @param context_id context ID
   * @param source_id 中断源 ID (1-1023，0 保留)
   * @param value 要设置的值
   */
  auto SetEnableBit(uint32_t context_id, uint32_t source_id, bool value) const
      -> void;

  /**
   * @brief 获取中断源优先级寄存器
   * @param source_id 中断源 ID (1-1023，0 保留)
   * @return uint32_t& 寄存器引用
   */
  auto SourcePriority(uint32_t source_id) const -> uint32_t&;

  /**
   * @brief 获取挂起位寄存器中指定中断源的状态
   * @param source_id 中断源 ID (1-1023，0 保留)
   * @return bool 对应位的状态
   */
  auto GetPendingBit(uint32_t source_id) const -> bool;

  /**
   * @brief 设置挂起位寄存器中指定中断源的状态
   * @param source_id 中断源 ID (1-1023，0 保留)
   * @param value 要设置的值
   */
  auto SetPendingBit(uint32_t source_id, bool value) const -> void;

  /**
   * @brief 获取优先级阈值寄存器
   * @param context_id context ID
   * @return uint32_t& 寄存器引用
   */
  auto PriorityThreshold(uint32_t context_id) const -> uint32_t&;

  /**
   * @brief 获取声明/完成寄存器
   * @param context_id context ID
   * @return uint32_t& 寄存器引用
   */
  auto ClaimComplete(uint32_t context_id) const -> uint32_t&;
};
#endif /* SIMPLEKERNEL_SRC_DRIVER_PLIC_INCLUDE_PLIC_H_ */
