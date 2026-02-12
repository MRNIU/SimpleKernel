/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_OPERATIONS_IRQ_CHIP_OPERATIONS_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_OPERATIONS_IRQ_CHIP_OPERATIONS_HPP_

#include "device_operations_base.hpp"

/**
 * @brief 中断触发类型标志位
 */
struct IrqType {
  uint32_t value;

  /// 使用硬件默认值
  static constexpr uint32_t kNone = 0;
  /// 上升沿触发
  static constexpr uint32_t kEdgeRising = 1U << 0;
  /// 下降沿触发
  static constexpr uint32_t kEdgeFalling = 1U << 1;
  static constexpr uint32_t kEdgeBoth = kEdgeRising | kEdgeFalling;
  /// 高电平触发
  static constexpr uint32_t kLevelHigh = 1U << 2;
  /// 低电平触发
  static constexpr uint32_t kLevelLow = 1U << 3;

  constexpr explicit IrqType(uint32_t v = 0) : value(v) {}

  constexpr auto operator|(IrqType other) const -> IrqType {
    return IrqType{value | other.value};
  }

  constexpr auto operator&(IrqType other) const -> IrqType {
    return IrqType{value & other.value};
  }

  constexpr explicit operator bool() const { return value != 0; }

  [[nodiscard]] constexpr auto IsEdge() const -> bool {
    return (value & (kEdgeRising | kEdgeFalling)) != 0;
  }

  [[nodiscard]] constexpr auto IsLevel() const -> bool {
    return (value & (kLevelHigh | kLevelLow)) != 0;
  }
};

/**
 * @brief 中断控制器抽象接口（参考 Linux struct irq_chip）
 *
 * 委托链: Startup → Enable → Unmask, Shutdown → Disable → Mask
 * 派生类至少覆写 DoMask/DoUnmask。
 *
 * @tparam Derived 具体中断控制器类型
 */
template <class Derived>
class IrqChipDevice : public DeviceOperationsBase<Derived> {
 public:
  /**
   * @brief 启动中断源
   * @param  irq  硬件中断号
   * @pre  irq 在控制器支持范围内
   * @post 中断源已初始化并使能
   */
  auto Startup(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoStartup(irq);
  }

  /**
   * @brief 关闭中断源
   * @param  irq  中断号
   * @post 中断源禁用，相关资源已释放
   */
  auto Shutdown(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoShutdown(irq);
  }

  /**
   * @brief 使能中断（默认委托给 Unmask）
   * @param  irq  中断号
   */
  auto Enable(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoEnable(irq);
  }

  /**
   * @brief 禁止中断（默认委托给 Mask）
   * @param  irq  中断号
   */
  auto Disable(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoDisable(irq);
  }

  /**
   * @brief 屏蔽中断
   * @param  irq  中断号
   */
  auto Mask(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoMask(irq);
  }

  /**
   * @brief 取消屏蔽中断
   * @param  irq  中断号
   */
  auto Unmask(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoUnmask(irq);
  }

  /**
   * @brief 应答中断（handler 入口处调用）
   * @param  irq  中断号
   */
  auto Ack(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoAck(irq);
  }

  /**
   * @brief 中断结束信号（handler 出口处调用）
   * @param  irq  中断号
   */
  auto Eoi(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoEoi(irq);
  }

  /**
   * @brief 设置中断触发类型
   * @param  irq   中断号
   * @param  type  触发类型
   */
  auto SetType(this Derived& self, uint32_t irq, IrqType type)
      -> Expected<void> {
    return self.DoSetType(irq, type);
  }

  /**
   * @brief 设置中断优先级
   * @param  irq       中断号
   * @param  priority  优先级值（含义取决于具体控制器）
   */
  auto SetPriority(this Derived& self, uint32_t irq, uint32_t priority)
      -> Expected<void> {
    return self.DoSetPriority(irq, priority);
  }

  /**
   * @brief 设置中断 CPU 亲和性
   * @param  irq       中断号
   * @param  cpu_mask  目标 CPU 掩码（bit N = CPU N）
   */
  auto SetAffinity(this Derived& self, uint32_t irq, uint64_t cpu_mask)
      -> Expected<void> {
    return self.DoSetAffinity(irq, cpu_mask);
  }

  /**
   * @brief 查询中断是否挂起
   * @param  irq  中断号
   * @return Expected<bool> true = 挂起
   */
  auto IsPending(this Derived& self, uint32_t irq) -> Expected<bool> {
    return self.DoIsPending(irq);
  }

  /**
   * @brief 向指定 CPU 发送 IPI
   * @param  cpu_id  目标 CPU 编号
   * @param  vector  中断向量号
   */
  auto SendIpi(this Derived& self, uint32_t cpu_id, uint8_t vector)
      -> Expected<void> {
    return self.DoSendIpi(cpu_id, vector);
  }

  /**
   * @brief 广播 IPI 到所有其他 CPU
   * @param  vector  中断向量号
   */
  auto BroadcastIpi(this Derived& self, uint8_t vector) -> Expected<void> {
    return self.DoBroadcastIpi(vector);
  }

  /**
   * @brief 初始化当前 CPU 的本地中断控制器
   */
  auto InitPerCpu(this Derived& self) -> Expected<void> {
    return self.DoInitPerCpu();
  }

 protected:
  // 默认 Startup/Shutdown 委托给 Enable/Disable
  auto DoStartup(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoEnable(irq);
  }

  auto DoShutdown(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoDisable(irq);
  }

  // 默认 Enable/Disable 委托给 Unmask/Mask
  auto DoEnable(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoUnmask(irq);
  }

  auto DoDisable(this Derived& self, uint32_t irq) -> Expected<void> {
    return self.DoMask(irq);
  }

  // 派生类至少覆写这两个
  auto DoMask([[maybe_unused]] uint32_t irq) -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  auto DoUnmask([[maybe_unused]] uint32_t irq) -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  auto DoAck([[maybe_unused]] uint32_t irq) -> Expected<void> { return {}; }

  auto DoEoi([[maybe_unused]] uint32_t irq) -> Expected<void> { return {}; }

  auto DoSetType([[maybe_unused]] uint32_t irq, [[maybe_unused]] IrqType type)
      -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  auto DoSetPriority([[maybe_unused]] uint32_t irq,
                     [[maybe_unused]] uint32_t priority) -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  auto DoSetAffinity([[maybe_unused]] uint32_t irq,
                     [[maybe_unused]] uint64_t cpu_mask) -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  auto DoIsPending([[maybe_unused]] uint32_t irq) -> Expected<bool> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  auto DoSendIpi([[maybe_unused]] uint32_t cpu_id,
                 [[maybe_unused]] uint8_t vector) -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  auto DoBroadcastIpi([[maybe_unused]] uint8_t vector) -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  auto DoInitPerCpu() -> Expected<void> { return {}; }
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_OPERATIONS_IRQ_CHIP_OPERATIONS_HPP_ */
