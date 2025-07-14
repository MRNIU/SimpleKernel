/**
 * @file interrupt.cpp
 * @brief 中断初始化
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-07-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-07-15<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#include "interrupt.h"

#include <cpu_io.h>

#include "basic_info.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "ns16550a.h"
#include "opensbi_interface.h"
#include "sk_cstdio"
#include "sk_iostream"

alignas(4) std::array<Interrupt::InterruptFunc,
                      cpu_io::detail::register_info::csr::ScauseInfo::
                          kInterruptMaxCount> Interrupt::interrupt_handlers_;
/// 异常处理函数数组
alignas(4) std::array<Interrupt::InterruptFunc,
                      cpu_io::detail::register_info::csr::ScauseInfo::
                          kExceptionMaxCount> Interrupt::exception_handlers_;

__attribute__((interrupt("supervisor"))) alignas(4) static void TarpEntry() {
  Singleton<Interrupt>::GetInstance().Do(
      static_cast<uint64_t>(cpu_io::Scause::Read()), nullptr);
}

Interrupt::Interrupt() {
  // 注册默认中断处理函数
  for (auto &i : interrupt_handlers_) {
    i = [](uint64_t cause, uint8_t *context) -> uint64_t {
      klog::Info("Default Interrupt handler [%s] 0x%X, 0x%p\n",
                 cpu_io::detail::register_info::csr::ScauseInfo::kInterruptNames
                     [cause],
                 cause, context);
      return 0;
    };
  }
  // 注册默认异常处理函数
  for (auto &i : exception_handlers_) {
    i = [](uint64_t cause, uint8_t *context) -> uint64_t {
      klog::Err("Default Exception handler [%s] 0x%X, 0x%p\n",
                cpu_io::detail::register_info::csr::ScauseInfo::kExceptionNames
                    [cause],
                cause, context);
      while (1);
      return 0;
    };
  }
  klog::Info("Interrupt init.\n");
}

void Interrupt::Do(uint64_t cause, uint8_t *context) {
  auto interrupt = cpu_io::Scause::Interrupt::Get(cause);
  auto exception_code = cpu_io::Scause::ExceptionCode::Get(cause);

  if (interrupt) {
    // 中断
    if (exception_code <
        cpu_io::detail::register_info::csr::ScauseInfo::kInterruptMaxCount) {
      interrupt_handlers_[exception_code](exception_code, context);
    }
  } else {
    // 异常
    if (exception_code <
        cpu_io::detail::register_info::csr::ScauseInfo::kExceptionMaxCount) {
      exception_handlers_[exception_code](exception_code, context);
    }
  }
}

void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptFunc func) {
  auto interrupt = cpu_io::Scause::Interrupt::Get(cause);
  auto exception_code = cpu_io::Scause::ExceptionCode::Get(cause);

  if (interrupt) {
    if (exception_code <
        cpu_io::detail::register_info::csr::ScauseInfo::kInterruptMaxCount) {
      interrupt_handlers_[exception_code] = func;
      klog::Info("RegisterInterruptFunc [%s] 0x%X, 0x%p\n",
                 cpu_io::detail::register_info::csr::ScauseInfo::kInterruptNames
                     [exception_code],
                 cause, func);
    }
  } else {
    if (exception_code <
        cpu_io::detail::register_info::csr::ScauseInfo::kExceptionMaxCount) {
      exception_handlers_[exception_code] = func;
      klog::Info("RegisterInterruptFunc [%s] 0x%X, 0x%p\n",
                 cpu_io::detail::register_info::csr::ScauseInfo::kExceptionNames
                     [exception_code],
                 cause, func);
    }
  }
}

static uint64_t kInterval = 0;

auto InterruptInit(int, const char **) -> int {
  // 获取 cpu 速度
  kInterval = Singleton<KernelFdt>::GetInstance().GetTimebaseFrequency();
  klog::Info("kInterval: 0x%X\n", kInterval);

  // 注册时钟中断
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kSupervisorTimerInterrupt,
      [](uint64_t exception_code, uint8_t *) -> uint64_t {
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
      [](uint64_t exception_code, uint8_t *) -> uint64_t {
        // 获取触发中断的源 ID
        auto source_id = Singleton<Plic>::GetInstance().Which();
        if (source_id != 0) {
          klog::Info("External interrupt from source %d\n", source_id);
          // 告知 PLIC 中断已处理完成
          Singleton<Plic>::GetInstance().Done(source_id);
        }
        klog::Info("Handle %s\n",
                   cpu_io::detail::register_info::csr::ScauseInfo::
                       kInterruptNames[exception_code]);
        return 0;
      });

  // ebreak 中断
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kBreakpoint,
      [](uint64_t exception_code, uint8_t *) -> uint64_t {
        cpu_io::Sepc::Write(cpu_io::Sepc::Read() + 2);
        klog::Info("Handle %s\n",
                   cpu_io::detail::register_info::csr::ScauseInfo::
                       kExceptionNames[exception_code]);
        return 0;
      });

  // 初始化 plic
  auto [plic_addr, plic_size, ndev, context_count] =
      Singleton<KernelFdt>::GetInstance().GetPlic();
  Singleton<Plic>::GetInstance() =
      std::move(Plic(plic_addr, ndev, context_count));

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

  asm("ebreak");

  // 设置时钟中断时间
  // sbi_set_timer(kInterval);

  klog::Info("Hello InterruptInit\n");

  Singleton<Plic>::GetInstance().Set(0, 10, 1, 1);
  Singleton<Plic>::GetInstance().Set(1, 10, 1, 1);

  auto [base, size] = Singleton<KernelFdt>::GetInstance().GetSerial();
  auto serial = Ns16550a(base);
  auto ret = serial.GetChar();
  klog::Info("Get char: %c\n", ret);

  while (1);

  // 唤醒其余 core
  for (size_t i = 0; i < Singleton<BasicInfo>::GetInstance().core_count; i++) {
    auto ret = sbi_hart_start(i, reinterpret_cast<uint64_t>(_boot), 0);
    if ((ret.error != SBI_SUCCESS) &&
        (ret.error != SBI_ERR_ALREADY_AVAILABLE)) {
      klog::Warn("hart %d start failed: %d\n", i, ret.error);
    }
  }

  return 0;
}

auto InterruptInitSMP(int, const char **) -> int {
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

  asm("ebreak");

  // 设置时钟中断时间
  sbi_set_timer(kInterval);

  klog::Info("Hello InterruptInitSMP\n");

  return 0;
}
