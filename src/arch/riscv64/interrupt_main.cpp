/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt.h"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "ns16550a.h"
#include "opensbi_interface.h"
#include "sk_cstdio"
#include "sk_iostream"
#include "syscall.hpp"
#include "virtual_memory.hpp"

namespace {
void RegisterInterrupts() {
  // 注册外部中断
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::
          kSupervisorExternalInterrupt,
      [](uint64_t, cpu_io::TrapContext*) -> uint64_t {
        // 获取触发中断的源 ID
        auto source_id = Singleton<Plic>::GetInstance().Which();
        Singleton<Plic>::GetInstance().Do(source_id, nullptr);
        Singleton<Plic>::GetInstance().Done(source_id);
        return 0;
      });

  auto [base, size, irq] =
      Singleton<KernelFdt>::GetInstance().GetSerial().value();
  Singleton<Ns16550a>::GetInstance() = std::move(Ns16550a(base));

  // 注册串口中断
  Singleton<Plic>::GetInstance().RegisterInterruptFunc(
      std::get<2>(Singleton<KernelFdt>::GetInstance().GetSerial().value()),
      [](uint64_t, uint8_t*) -> uint64_t {
        sk_putchar(Singleton<Ns16550a>::GetInstance().TryGetChar(), nullptr);
        return 0;
      });

  // 注册 ebreak 中断
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kBreakpoint,
      [](uint64_t exception_code, cpu_io::TrapContext* context) -> uint64_t {
        // 读取 sepc 处的指令
        auto instruction = *reinterpret_cast<uint8_t*>(context->sepc);

        // 判断是否为压缩指令 (低 2 位不为 11)
        if ((instruction & 0x3) != 0x3) {
          // 2 字节指令
          context->sepc += 2;
        } else {
          // 4 字节指令
          context->sepc += 4;
        }
        klog::Info("Handle %s\n",
                   cpu_io::detail::register_info::csr::ScauseInfo::
                       kExceptionNames[exception_code]);
        return 0;
      });

  // 注册缺页中断处理
  auto page_fault_handler = [](uint64_t exception_code,
                               cpu_io::TrapContext* context) -> uint64_t {
    auto addr = cpu_io::Stval::Read();
    klog::Err("PageFault: %s(0x%lx), addr: 0x%lx\n",
              cpu_io::detail::register_info::csr::ScauseInfo::kExceptionNames
                  [exception_code],
              exception_code, addr);
    klog::Err("sepc: 0x%lx\n", context->sepc);
    DumpStack();
    while (1) {
      cpu_io::Pause();
    }
    return 0;
  };

  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kInstructionPageFault,
      page_fault_handler);
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kLoadPageFault,
      page_fault_handler);
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kStoreAmoPageFault,
      page_fault_handler);

  // 注册系统调用
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kEcallUserMode,
      [](uint64_t, cpu_io::TrapContext* context) -> uint64_t {
        Syscall(0, context);
        return 0;
      });

  // 注册软中断 (IPI)
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::
          kSupervisorSoftwareInterrupt,
      [](uint64_t, cpu_io::TrapContext*) -> uint64_t {
        // 清软中断 pending 位
        cpu_io::Sip::Ssip::Clear();
        klog::Debug("Core %d received IPI\n", cpu_io::GetCurrentCoreId());
        return 0;
      });
}

}  // namespace

extern "C" cpu_io::TrapContext* HandleTrap(cpu_io::TrapContext* context) {
  Singleton<Interrupt>::GetInstance().Do(context->scause, context);
  return context;
}

void InterruptInit(int, const char**) {
  // 注册中断处理函数
  RegisterInterrupts();

  // 初始化 plic
  auto [plic_addr, plic_size, ndev, context_count] =
      Singleton<KernelFdt>::GetInstance().GetPlic().value();
  Singleton<VirtualMemory>::GetInstance().MapMMIO(plic_addr, plic_size);
  Singleton<Plic>::GetInstance() =
      std::move(Plic(plic_addr, ndev, context_count));

  // 设置 trap vector
  auto success =
      cpu_io::Stvec::SetDirect(reinterpret_cast<uint64_t>(trap_entry));
  if (!success) {
    klog::Err("Failed to set trap vector\n");
  }

  // 开启 Supervisor 中断
  cpu_io::Sstatus::Sie::Set();

  // 开启内部中断
  cpu_io::Sie::Ssie::Set();

  // 开启外部中断
  cpu_io::Sie::Seie::Set();

  // 为当前 core 开启串口中断
  Singleton<Plic>::GetInstance().Set(
      cpu_io::GetCurrentCoreId(),
      std::get<2>(Singleton<KernelFdt>::GetInstance().GetSerial().value()), 1,
      true);

  // 初始化定时器
  TimerInit();

  klog::Info("Hello InterruptInit\n");
}

void InterruptInitSMP(int, const char**) {
  // 设置 trap vector
  auto success =
      cpu_io::Stvec::SetDirect(reinterpret_cast<uint64_t>(trap_entry));
  if (!success) {
    klog::Err("Failed to set trap vector\n");
  }

  // 开启 Supervisor 中断
  cpu_io::Sstatus::Sie::Set();

  // 开启内部中断
  cpu_io::Sie::Ssie::Set();

  // 开启外部中断
  cpu_io::Sie::Seie::Set();

  // 初始化定时器
  TimerInitSMP();

  klog::Info("Hello InterruptInitSMP\n");
}
