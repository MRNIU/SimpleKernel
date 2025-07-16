
/**
 * @file gic.h
 * @brief gic 头文件
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

#include "gic.h"

#include "io.hpp"
#include "kernel_log.hpp"

Gic::Gic(uint64_t gicd_base_addr, uint64_t gicr_base_addr)
    : gicd_(gicd_base_addr), gicr_(gicr_base_addr) {
  cpu_io::ICC_IGRPEN1_EL1::Enable::Clear();
  cpu_io::ICC_PMR_EL1::Priority::Set();

  gicd_.EnableGrp1NS();

  klog::Info("Gic init.\n");
}

void Gic::SetUP() const {
  cpu_io::ICC_IGRPEN1_EL1::Enable::Clear();
  cpu_io::ICC_PMR_EL1::Priority::Set();
  gicd_.EnableGrp1NS();

  gicr_.SetUP();
}

void Gic::SPI(uint32_t intid, uint32_t cpuid) const {
  gicd_.SetupSPI(intid, cpuid);
}

void Gic::PPI(uint32_t intid, uint32_t cpuid) const {
  gicr_.SetupPPI(intid, cpuid);
}

Gic::Gicd::Gicd(uint64_t base_addr) : base_addr_(base_addr) {
  if (base_addr_ == 0) {
    klog::Err("GICD base address is invalid [0x%X]\n", base_addr_);
    throw;
  }
  // 将 GICD_CTLR 清零
  Write(kCTLR, 0);

  // 读取 ITLinesNumber 数量
  auto it_lines_number = Read(kTYPER) & kTYPER_ITLinesNumberMask;

  klog::Info("it_lines_number %d\n", it_lines_number);

  // 设置中断为 Non-secure Group 1
  for (uint32_t i = 0; i < it_lines_number; i++) {
    Write(IGROUPRn(i), UINT32_MAX);
  }
}

void Gic::Gicd::Enable(uint32_t intid) const {
  auto is = Read(ISENABLERn(intid / kISENABLERn_SIZE));
  is |= 1 << (intid % kISENABLERn_SIZE);
  Write(ISENABLERn(intid / kISENABLERn_SIZE), is);
}

void Gic::Gicd::Disable(uint32_t intid) const {
  auto ic = Read(ICENABLERn(intid / kICENABLERn_SIZE));
  ic |= 1 << (intid % kICENABLERn_SIZE);
  Write(ICENABLERn(intid / kICENABLERn_SIZE), ic);
}

void Gic::Gicd::Clear(uint32_t intid) const {
  auto ic = Read(ICPENDRn(intid / kICPENDRn_SIZE));
  ic |= 1 << (intid % kICPENDRn_SIZE);
  Write(ICPENDRn(intid / kICPENDRn_SIZE), ic);
}

auto Gic::Gicd::IsEnable(uint32_t intid) const -> bool {
  auto is = Read(ISENABLERn(intid / kISENABLERn_SIZE));
  return is & (1 << (intid % kISENABLERn_SIZE));
}

void Gic::Gicd::SetPrio(uint32_t intid, uint32_t prio) const {
  auto shift = (intid % kIPRIORITYRn_SIZE) * kIPRIORITYRn_BITS;
  auto ip = Read(IPRIORITYRn(intid / kIPRIORITYRn_SIZE));
  ip &= ~(kIPRIORITYRn_BITS_MASK << shift);
  ip |= prio << shift;
  Write(IPRIORITYRn(intid / kIPRIORITYRn_SIZE), ip);
}

void Gic::Gicd::SetConfig(uint32_t intid, uint32_t config) const {
  auto shift = (intid % kICFGRn_SIZE) * kICFGRn_BITS;
  auto ic = Read(ICFGRn(intid / kICFGRn_SIZE));
  ic &= ~(kICFGRn_BITS_MASK << shift);
  ic |= config << shift;
  Write(ICFGRn(intid / kICFGRn_SIZE), ic);
}

void Gic::Gicd::SetTarget(uint32_t intid, uint32_t cpuid) const {
  auto target = Read(ITARGETSRn(intid / kITARGETSRn_SIZE));
  target &=
      ~(kICFGRn_BITS_MASK << ((intid % kITARGETSRn_SIZE) * kITARGETSRn_BITS));
  Write(ITARGETSRn(intid / kITARGETSRn_SIZE),
        target |
            ((1 << cpuid) << ((intid % kITARGETSRn_SIZE) * kITARGETSRn_BITS)));
}

void Gic::Gicr::SetUP() const {
  auto cpuid = cpu_io::GetCurrentCoreId();

  // 将 GICR_CTLR 清零
  Write(cpuid, kCTLR, 0);

  // The System register interface for the current Security state is enabled.
  cpu_io::ICC_SRE_EL1::SRE::Set();

  // 允许 Non-secure Group 1 中断
  Write(cpuid, kIGROUPR0, kIGROUPR0_Set);
  Write(cpuid, kIGRPMODR0, kIGRPMODR0_Clear);

  // 唤醒 Redistributor
  // @see
  // https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-WAKER--Redistributor-Wake-Register?lang=en
  auto waker = Read(cpuid, kWAKER);
  // Clear the ProcessorSleep bit
  Write(cpuid, kWAKER, waker & ~kWAKER_ProcessorSleepMASK);
  // 等待唤醒完成
  while (Read(cpuid, kWAKER) & kWAKER_ChildrenAsleepMASK) {
    ;
  }
}

void Gic::Gicd::SetupSPI(uint32_t intid, uint32_t cpuid) const {
  // 电平触发
  SetConfig(intid, kICFGRn_LevelSensitive);

  // 优先级设定
  SetPrio(intid, 0);

  // 设置所有中断由 cpu0 处理
  SetTarget(intid, cpuid);
  // 清除中断请求
  Clear(intid);
  // 使能中断
  Enable(intid);
}

Gic::Gicr::Gicr(uint64_t base_addr) : base_addr_(base_addr) {
  if (base_addr_ == 0) {
    klog::Err("GICR base address is invalid [0x%X]\n", base_addr_);
    throw;
  }

  auto cpuid = cpu_io::GetCurrentCoreId();

  // 将 GICR_CTLR 清零
  Write(cpuid, kCTLR, 0);

  // The System register interface for the current Security state is enabled.
  cpu_io::ICC_SRE_EL1::SRE::Set();

  // 允许 Non-secure Group 1 中断
  Write(cpuid, kIGROUPR0, kIGROUPR0_Set);
  Write(cpuid, kIGRPMODR0, kIGRPMODR0_Clear);

  // 唤醒 Redistributor
  // @see
  // https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-WAKER--Redistributor-Wake-Register?lang=en
  auto waker = Read(cpuid, kWAKER);
  // Clear the ProcessorSleep bit
  Write(cpuid, kWAKER, waker & ~kWAKER_ProcessorSleepMASK);
  // 等待唤醒完成
  while (Read(cpuid, kWAKER) & kWAKER_ChildrenAsleepMASK) {
    ;
  }
}

void Gic::Gicr::Enable(uint32_t intid, uint32_t cpuid) const {
  auto is = Read(cpuid, kISENABLER0);
  is |= 1 << (intid % kISENABLER0_SIZE);
  Write(cpuid, kISENABLER0, is);
}

void Gic::Gicr::Disable(uint32_t intid, uint32_t cpuid) const {
  auto ic = Read(cpuid, kICENABLER0);
  ic |= 1 << (intid % kICENABLER0_SIZE);
  Write(cpuid, kICENABLER0, ic);
}

void Gic::Gicr::Clear(uint32_t intid, uint32_t cpuid) const {
  auto ic = Read(cpuid, kICPENDR0);
  ic |= 1 << (intid % kICPENDR0_SIZE);
  Write(cpuid, kICPENDR0, ic);
}

void Gic::Gicr::SetPrio(uint32_t intid, uint32_t cpuid, uint32_t prio) const {
  auto shift = (intid % kIPRIORITYRn_SIZE) * kIPRIORITYRn_BITS;
  auto ip = Read(cpuid, IPRIORITYRn(intid / kIPRIORITYRn_SIZE));
  ip &= ~(kIPRIORITYRn_BITS_MASK << shift);
  ip |= prio << shift;
  Write(cpuid, IPRIORITYRn(intid / kIPRIORITYRn_SIZE), ip);
}

void Gic::Gicr::SetupPPI(uint32_t intid, uint32_t cpuid) const {
  SetPrio(intid, cpuid, 0);
  Clear(intid, cpuid);
  Enable(intid, cpuid);
}
