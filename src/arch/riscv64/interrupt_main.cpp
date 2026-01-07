/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断初始化
 */

#include <cpu_io.h>

#include "basic_info.hpp"
#include "interrupt.h"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "ns16550a.h"
#include "opensbi_interface.h"
#include "sk_cstdio"
#include "sk_iostream"
#include "virtual_memory.hpp"

extern "C" void trap_entry();

extern "C" void HandleTrap(cpu_io::TrapContext* context) {
  Singleton<Interrupt>::GetInstance().Do(context->scause,
                                         reinterpret_cast<uint8_t*>(context));
}

namespace {
uint64_t kInterval = 0;
}  // namespace

void InterruptInit(int, const char**) {
  // 获取 cpu 速度
  kInterval = Singleton<KernelFdt>::GetInstance().GetTimebaseFrequency();
  klog::Info("kInterval: 0x%X\n", kInterval);

  // 注册时钟中断
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kSupervisorTimerInterrupt,
      [](uint64_t exception_code, uint8_t*) -> uint64_t {
        sbi_set_timer(cpu_io::Time::Read() + kInterval);
        klog::Info("Handle %s\n",
                   cpu_io::detail::register_info::csr::ScauseInfo::
                       kInterruptNames[exception_code]);
        return 0;
      });

  // 注册外部中断
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::
          kSupervisorExternalInterrupt,
      [](uint64_t exception_code, uint8_t*) -> uint64_t {
        // 获取触发中断的源 ID
        auto source_id = Singleton<Plic>::GetInstance().Which();
        klog::Debug("External interrupt from source %d\n", source_id);
        Singleton<Plic>::GetInstance().Do(source_id, nullptr);
        Singleton<Plic>::GetInstance().Done(source_id);
        klog::Debug("Handle %s\n",
                    cpu_io::detail::register_info::csr::ScauseInfo::
                        kInterruptNames[exception_code]);
        return 0;
      });

  auto [base, size, irq] = Singleton<KernelFdt>::GetInstance().GetSerial();
  Singleton<Ns16550a>::GetInstance() = std::move(Ns16550a(base));

  // 注册串口中断
  Singleton<Plic>::GetInstance().RegisterInterruptFunc(
      std::get<2>(Singleton<KernelFdt>::GetInstance().GetSerial()),
      [](uint64_t, uint8_t*) -> uint64_t {
        sk_putchar(Singleton<Ns16550a>::GetInstance().TryGetChar(), nullptr);
        return 0;
      });

  // ebreak 中断
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kBreakpoint,
      [](uint64_t exception_code, uint8_t* context) -> uint64_t {
        auto* trap_context = reinterpret_cast<cpu_io::TrapContext*>(context);
        klog::Debug("scause: 0x%X\n", trap_context->scause);
        klog::Debug("sepc: 0x%X\n", trap_context->sepc);
        klog::Debug("sp: 0x%X\n", trap_context->sp);
        klog::Debug("sstatus: 0x%X\n", trap_context->sstatus);
        klog::Debug("stval: 0x%X\n", trap_context->stval);

        // 读取 sepc 处的指令
        auto instruction = *reinterpret_cast<uint8_t*>(trap_context->sepc);

        // 判断是否为压缩指令 (低 2 位不为 11)
        if ((instruction & 0x3) != 0x3) {
          // 2 字节指令
          trap_context->sepc += 2;
        } else {
          // 4 字节指令
          trap_context->sepc += 4;
        }
        klog::Info("Handle %s\n",
                   cpu_io::detail::register_info::csr::ScauseInfo::
                       kExceptionNames[exception_code]);
        return 0;
      });

  // 初始化 plic
  auto [plic_addr, plic_size, ndev, context_count] =
      Singleton<KernelFdt>::GetInstance().GetPlic();
  Singleton<VirtualMemory>::GetInstance().MapMMIO(plic_addr, plic_size);
  Singleton<Plic>::GetInstance() =
      std::move(Plic(plic_addr, ndev, context_count));

  // 为当前 core 开启串口中断
  Singleton<Plic>::GetInstance().Set(
      cpu_io::GetCurrentCoreId(),
      std::get<2>(Singleton<KernelFdt>::GetInstance().GetSerial()), 1, true);

  // 设置 trap vector
  cpu_io::Stvec::SetDirect(reinterpret_cast<uint64_t>(trap_entry));

  // 开启 Supervisor 中断
  cpu_io::Sstatus::Sie::Set();

  // 开启内部中断
  cpu_io::Sie::Ssie::Set();

  // 开启时钟中断
  cpu_io::Sie::Stie::Set();

  // 开启外部中断
  cpu_io::Sie::Seie::Set();

  // 设置时钟中断时间
  sbi_set_timer(kInterval);

  klog::Info("Hello InterruptInit\n");
}

void InterruptInitSMP(int, const char**) {
  // 设置 trap vector
  cpu_io::Stvec::SetDirect(reinterpret_cast<uint64_t>(trap_entry));

  // 开启 Supervisor 中断
  cpu_io::Sstatus::Sie::Set();

  // 开启内部中断
  cpu_io::Sie::Ssie::Set();

  // 开启时钟中断
  cpu_io::Sie::Stie::Set();

  // 开启外部中断
  cpu_io::Sie::Seie::Set();

  // 设置时钟中断时间
  sbi_set_timer(kInterval);

  klog::Info("Hello InterruptInitSMP\n");
}
