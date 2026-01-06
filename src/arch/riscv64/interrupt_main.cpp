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

namespace {
uint64_t kInterval = 0;
__attribute__((interrupt("supervisor"))) alignas(4) void TarpEntry() {
  Singleton<Interrupt>::GetInstance().Do(
      static_cast<uint64_t>(cpu_io::Scause::Read()), nullptr);
}
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
      [](uint64_t exception_code, uint8_t*) -> uint64_t {
        cpu_io::Sepc::Write(cpu_io::Sepc::Read() + 2);
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
  cpu_io::Stvec::SetDirect(reinterpret_cast<uint64_t>(TarpEntry));

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

  // 唤醒其余 core
  for (size_t i = 0; i < Singleton<BasicInfo>::GetInstance().core_count; i++) {
    auto ret = sbi_hart_start(i, reinterpret_cast<uint64_t>(_boot), 0);
    if ((ret.error != SBI_SUCCESS) &&
        (ret.error != SBI_ERR_ALREADY_AVAILABLE)) {
      klog::Warn("hart %d start failed: %d\n", i, ret.error);
    }
  }
}

void InterruptInitSMP(int, const char**) {
  // 设置 trap vector
  cpu_io::Stvec::SetDirect(reinterpret_cast<uint64_t>(TarpEntry));

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
