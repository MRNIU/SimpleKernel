/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <device_framework/ns16550a.hpp>

#include "arch.h"
#include "basic_info.hpp"
#include "driver/virtio_blk_driver.hpp"
#include "driver_registry.hpp"
#include "interrupt.h"
#include "kernel.h"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "kstd_cstdio"
#include "kstd_iostream"
#include "opensbi_interface.h"
#include "platform_traits.hpp"
#include "syscall.hpp"
#include "task_manager.hpp"
#include "virtual_memory.hpp"

using Ns16550aSingleton =
    etl::singleton<device_framework::ns16550a::Ns16550aDevice>;
using InterruptDelegate = InterruptBase::InterruptDelegate;
namespace {
// 外部中断分发器：CPU 外部中断 -> PLIC -> 设备 handler
auto ExternalInterruptHandler(uint64_t /*cause*/, cpu_io::TrapContext* context)
    -> uint64_t {
  auto& plic = InterruptSingleton::instance().plic();
  auto source_id = plic.Which();
  plic.Do(source_id, context);
  plic.Done(source_id);
  return 0;
}

// ebreak 中断处理
auto EbreakHandler(uint64_t exception_code, cpu_io::TrapContext* context)
    -> uint64_t {
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
  klog::Info("Handle %s\n", cpu_io::detail::register_info::csr::ScauseInfo::
                                kExceptionNames[exception_code]);
  return 0;
}

// 缺页中断处理
auto PageFaultHandler(uint64_t exception_code, cpu_io::TrapContext* context)
    -> uint64_t {
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
}

// 系统调用处理
auto SyscallHandler(uint64_t /*cause*/, cpu_io::TrapContext* context)
    -> uint64_t {
  Syscall(0, context);
  return 0;
}

// 软中断 (IPI) 处理
auto IpiHandler(uint64_t /*cause*/, cpu_io::TrapContext* /*context*/)
    -> uint64_t {
  // 清软中断 pending 位
  cpu_io::Sip::Ssip::Clear();
  klog::Debug("Core %d received IPI\n", cpu_io::GetCurrentCoreId());
  return 0;
}

// 串口外部中断处理
auto SerialIrqHandler(uint64_t /*cause*/, cpu_io::TrapContext* /*context*/)
    -> uint64_t {
  Ns16550aSingleton::instance().HandleInterrupt(
      [](uint8_t ch) { sk_putchar(ch, nullptr); });
  return 0;
}

// VirtIO-blk 外部中断处理
auto VirtioBlkIrqHandler(uint64_t /*cause*/, cpu_io::TrapContext* /*context*/)
    -> uint64_t {
  DriverRegistry::GetDriverInstance<VirtioBlkDriver>().HandleInterrupt(
      [](void* /*token*/, device_framework::ErrorCode status) {
        if (status != device_framework::ErrorCode::kSuccess) {
          klog::Err("VirtIO blk IO error: %d\n", static_cast<int>(status));
        }
      });
  return 0;
}

void RegisterInterrupts() {
  // 注册外部中断分发器：CPU 外部中断 -> PLIC -> 设备 handler
  InterruptSingleton::instance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::
          kSupervisorExternalInterrupt,
      InterruptDelegate::create<ExternalInterruptHandler>());

  auto [base, size, irq] = KernelFdtSingleton::instance().GetSerial().value();
  auto uart_result = device_framework::ns16550a::Ns16550aDevice::Create(base);
  if (uart_result) {
    Ns16550aSingleton::create(std::move(*uart_result));
  } else {
    klog::Err("Failed to create Ns16550aDevice: %d\n",
              static_cast<int>(uart_result.error().code));
  }
  Ns16550aSingleton::instance().OpenReadWrite().or_else(
      [](device_framework::Error e) -> device_framework::Expected<void> {
        klog::Err(
            "Failed to open device_framework::ns16550a::Ns16550aDevice: %d\n",
            static_cast<int>(e.code));
        return device_framework::Expected<void>{};
      });

  // 注册 ebreak 中断
  InterruptSingleton::instance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kBreakpoint,
      InterruptDelegate::create<EbreakHandler>());

  // 注册缺页中断处理
  InterruptSingleton::instance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kInstructionPageFault,
      InterruptDelegate::create<PageFaultHandler>());
  InterruptSingleton::instance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kLoadPageFault,
      InterruptDelegate::create<PageFaultHandler>());
  InterruptSingleton::instance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kStoreAmoPageFault,
      InterruptDelegate::create<PageFaultHandler>());

  // 注册系统调用
  InterruptSingleton::instance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kEcallUserMode,
      InterruptDelegate::create<SyscallHandler>());

  // 注册软中断 (IPI)
  InterruptSingleton::instance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::
          kSupervisorSoftwareInterrupt,
      InterruptDelegate::create<IpiHandler>());
}

}  // namespace

extern "C" cpu_io::TrapContext* HandleTrap(cpu_io::TrapContext* context) {
  InterruptSingleton::instance().Do(context->scause, context);
  return context;
}

void InterruptInit(int, const char**) {
  InterruptSingleton::create();

  // 注册中断处理函数
  // 注册中断处理函数
  RegisterInterrupts();

  // 初始化 plic
  auto [plic_addr, plic_size, ndev, context_count] =
      KernelFdtSingleton::instance().GetPlic().value();
  VirtualMemorySingleton::instance().MapMMIO(plic_addr, plic_size);
  InterruptSingleton::instance().InitPlic(plic_addr, ndev, context_count);

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

  // 通过统一接口注册串口外部中断（先注册 handler，再启用 PLIC）
  auto serial_irq =
      std::get<2>(KernelFdtSingleton::instance().GetSerial().value());
  InterruptSingleton::instance()
      .RegisterExternalInterrupt(serial_irq, cpu_io::GetCurrentCoreId(), 1,
                                 InterruptDelegate::create<SerialIrqHandler>())
      .or_else([](Error err) -> Expected<void> {
        klog::Err("Failed to register serial IRQ: %s\n", err.message());
        return std::unexpected(err);
      });

  // 通过统一接口注册 virtio-blk 外部中断
  using BlkDriver = VirtioBlkDriver;
  auto& blk_driver = DriverRegistry::GetDriverInstance<BlkDriver>();
  auto blk_irq = blk_driver.GetIrq();
  if (blk_irq != 0) {
    InterruptSingleton::instance()
        .RegisterExternalInterrupt(
            blk_irq, cpu_io::GetCurrentCoreId(), 1,
            InterruptDelegate::create<VirtioBlkIrqHandler>())
        .or_else([blk_irq](Error err) -> Expected<void> {
          klog::Err("Failed to register virtio-blk IRQ %u: %s\n", blk_irq,
                    err.message());
          return std::unexpected(err);
        });
  }

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
