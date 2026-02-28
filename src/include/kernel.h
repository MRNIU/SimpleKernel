/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_H_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_H_

#include <etl/singleton.h>

#include <cstdint>

#include "basic_info.hpp"
#include "device_manager.hpp"
#include "interrupt.h"
#include "kernel_elf.hpp"
#include "kernel_fdt.hpp"
#include "task_manager.hpp"
#include "virtual_memory.hpp"

#if defined(__x86_64__)
#include <cpu_io.h>
#elif defined(__riscv)
#include <device_framework/ns16550a.hpp>
#elif defined(__aarch64__)
#include <device_framework/pl011.hpp>
#endif

// ---------------------------------------------------------------------------
// Singleton type aliases
// ---------------------------------------------------------------------------

using BasicInfoSingleton = etl::singleton<BasicInfo>;
using KernelFdtSingleton = etl::singleton<KernelFdt>;
using KernelElfSingleton = etl::singleton<KernelElf>;
using VirtualMemorySingleton = etl::singleton<VirtualMemory>;
using TaskManagerSingleton = etl::singleton<TaskManager>;
using DeviceManagerSingleton = etl::singleton<DeviceManager>;
using InterruptSingleton = etl::singleton<Interrupt>;

#if defined(__x86_64__)
using ApicSingleton = etl::singleton<Apic>;
using SerialSingleton = etl::singleton<cpu_io::Serial>;
#elif defined(__riscv)
using PlicSingleton = etl::singleton<Plic>;
using Ns16550aSingleton =
    etl::singleton<device_framework::ns16550a::Ns16550aDevice>;
#elif defined(__aarch64__)
using Pl011Singleton = etl::singleton<device_framework::pl011::Pl011Device>;
#endif

// ---------------------------------------------------------------------------
// Kernel entry points
// ---------------------------------------------------------------------------

/**
 * @brief 负责 crtbegin 的工作
 * @param argc
 *          riscv64: 启动核 id
 * @param argv 参数指针
 *          riscv64: dtb 地址
 * @return uint32_t 正常返回 0
 */
extern "C" [[maybe_unused]] [[noreturn]] void _start(int argc,
                                                     const char** argv);

/**
 * @brief 内核入口
 * @param argc 同 _start
 * @param argv 同 _start
 * @return int 正常返回 0
 */
auto main(int argc, const char** argv) -> int;

void MemoryInit();
void MemoryInitSMP();

void DeviceInit();

void FileSystemInit();

#endif /* SIMPLEKERNEL_SRC_INCLUDE_KERNEL_H_ */
