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

#include "plic.h"

#include <cpu_io.h>

#include "basic_info.hpp"
#include "kernel_log.hpp"
#include "sk_string.h"

alignas(
    4) std::array<Plic::InterruptFunc,
                  Plic::kInterruptMaxCount> Plic::external_interrupt_handlers_;

Plic::Plic(uint64_t dev_addr, size_t ndev, size_t context_count)
    : base_addr_(dev_addr), ndev_(ndev), context_count_(context_count) {
  // 设置所有中断源的优先级为 0 (禁用)
  klog::Info("Setting all interrupt priorities to 0\n");
  for (size_t source_id = 0; source_id <= ndev_; source_id++) {
    SourcePriority(source_id) = 0;
  }

  klog::Info("Disabling all interrupts for all contexts\n");

  for (size_t context_id = 0; context_id < context_count_; context_id++) {
    // 设置优先级阈值为 0 (允许所有优先级的中断)
    PriorityThreshold(context_id) = 0;
    // 禁用所有中断
    for (size_t source_id = 0; source_id <= ndev_; source_id++) {
      SetEnableBit(context_id, source_id, false);
    }
  }

  klog::Info(
      "PLIC initialization: all interrupts disabled, priorities set to 0\n");
}

auto Plic::Which() const -> uint32_t {
  auto context_id = GetContextId(cpu_io::GetCurrentCoreId());
  return ClaimComplete(context_id);
}

void Plic::Done(uint32_t source_id) const {
  auto context_id = GetContextId(cpu_io::GetCurrentCoreId());
  ClaimComplete(context_id) = source_id;
}

// void Plic::register_externel_handler(
//     uint8_t cause, InterruptFunc _interrupt_handler) {
//   externel_interrupt_handlers[cause] = _interrupt_handler;
//   return;
// }

// void Plic::do_externel_interrupt(uint8_t cause) {
//   externel_interrupt_handlers[cause](cause);
//   return;
// }

void Plic::Set(uint32_t hart_id, uint32_t source_id, uint32_t priority,
               bool enable) const {
  // 设置中断优先级
  SourcePriority(source_id) = priority;

  // 设置中断使能状态
  uint32_t context_id = GetContextId(hart_id);
  SetEnableBit(context_id, source_id, enable);
}

auto Plic::Get(uint32_t hart_id, uint32_t source_id) const
    -> std::tuple<uint32_t, bool, bool> {
  // 获取中断优先级
  auto priority = SourcePriority(source_id);

  // 获取中断使能状态
  uint32_t context_id = GetContextId(hart_id);
  auto enabled = GetEnableBit(context_id, source_id);

  // 获取中断挂起状态
  auto pending = GetPendingBit(source_id);

  return {priority, enabled, pending};
}

auto Plic::GetEnableBit(uint32_t context_id, uint32_t source_id) const -> bool {
  uint32_t word_index = source_id / 32;
  uint32_t bit_index = source_id % 32;
  uint32_t* word_addr =
      (uint32_t*)(base_addr_ + kEnableBitsOffset + (context_id * kEnableSize) +
                  (word_index * 4));
  return (*word_addr >> bit_index) & 1;
}

auto Plic::SetEnableBit(uint32_t context_id, uint32_t source_id,
                        bool value) const -> void {
  uint32_t word_index = source_id / 32;
  uint32_t bit_index = source_id % 32;
  uint32_t* word_addr =
      (uint32_t*)(base_addr_ + kEnableBitsOffset + (context_id * kEnableSize) +
                  (word_index * 4));
  if (value) {
    *word_addr |= (1U << bit_index);
  } else {
    *word_addr &= ~(1U << bit_index);
  }
}

auto Plic::SourcePriority(uint32_t source_id) const -> uint32_t& {
  return *(uint32_t*)(base_addr_ + kSourcePriorityOffset +
                      (source_id * sizeof(uint32_t)));
}

auto Plic::GetPendingBit(uint32_t source_id) const -> bool {
  uint32_t word_index = source_id / 32;
  uint32_t bit_index = source_id % 32;
  uint32_t* word_addr =
      (uint32_t*)(base_addr_ + kPendingBitsOffset + (word_index * 4));
  return (*word_addr >> bit_index) & 1;
}

auto Plic::SetPendingBit(uint32_t source_id, bool value) const -> void {
  uint32_t word_index = source_id / 32;
  uint32_t bit_index = source_id % 32;
  uint32_t* word_addr =
      (uint32_t*)(base_addr_ + kPendingBitsOffset + (word_index * 4));
  if (value) {
    *word_addr |= (1U << bit_index);
  } else {
    *word_addr &= ~(1U << bit_index);
  }
}

auto Plic::PriorityThreshold(uint32_t context_id) const -> uint32_t& {
  return *(uint32_t*)(base_addr_ + kContextOffset +
                      (context_id * kContextSize) + kPriorityThresholdOffset);
}

auto Plic::ClaimComplete(uint32_t context_id) const -> uint32_t& {
  return *(uint32_t*)(base_addr_ + kContextOffset +
                      (context_id * kContextSize) + kClaimCompleteOffset);
}
