// Copyright The SimpleKernel Contributors

/// 早期初始化（架构无关）——解析 FDT，将信息分发到各子系统。
///
/// 在堆和分页启用之前运行，仅依赖 logging 和栈。
pub fn early_init(dtb_addr: usize) {
    use crate::CORE_COUNT;
    use memory::{MEMORY_INFO, MemoryInfo};
    use memory_types::PhysAddr;

    // SAFETY: dtb_addr 来自架构启动入口；bootloader 契约保证它在 early_init 期间可读。
    let fdt = match unsafe { platform_fdt::init_from_raw(dtb_addr) } {
        Ok(fdt) => fdt,
        Err(error) => {
            log::error!("PlatformFdt: {}", error);
            crate::util::halt::halt("无法初始化内核自有 DTB 副本");
        }
    };

    let node_count = match fdt.node_count() {
        Ok(count) => count,
        Err(error) => {
            log::error!("PlatformFdt: FDT 节点计数失败: {}", error);
            crate::util::halt::halt("无法遍历 FDT 节点");
        }
    };
    let core_count = crate::cpu_topology::init_from_fdt(fdt);

    let (mem_addr, mem_size) = match fdt.memory() {
        Ok(m) => m,
        Err(error) => {
            log::error!("PlatformFdt: FDT memory 解析失败: {}", error);
            crate::util::halt::halt("无法从 FDT 获取内存信息");
        }
    };

    // SAFETY: 链接器定义的符号，地址在内核生命周期内有效
    unsafe extern "C" {
        static __executable_start: u8;
        static _end: u8;
    }
    let kernel_start = unsafe { &__executable_start as *const u8 as u64 };
    let kernel_end = unsafe { &_end as *const u8 as u64 };

    let firmware_reserved = match fdt.firmware_reserved_memory() {
        Ok(region) => region,
        Err(platform_fdt::FdtError::NodeNotFound) => {
            let size = kernel_start
                .checked_sub(mem_addr)
                .expect("early_init: kernel_start 小于 FDT RAM 起点");
            (mem_addr, size as usize)
        }
        Err(error) => {
            log::error!("PlatformFdt: firmware reserved-memory 解析失败: {}", error);
            crate::util::halt::halt("无法从 FDT 获取固件保留区");
        }
    };

    MEMORY_INFO.call_once(|| MemoryInfo {
        physical_memory_addr: PhysAddr::new(mem_addr as usize),
        physical_memory_size: mem_size,
        kernel_addr: PhysAddr::new(kernel_start as usize),
        kernel_size: (kernel_end - kernel_start) as usize,
        firmware_reserved_addr: PhysAddr::new(firmware_reserved.0 as usize),
        firmware_reserved_size: firmware_reserved.1,
    });

    CORE_COUNT.call_once(|| core_count);

    // RISC-V 的 timebase-frequency 在 FDT /cpus 节点中；
    // aarch64 从 CNTFRQ_EL0 寄存器直接读取，不需要此值。
    #[cfg(target_arch = "riscv64")]
    {
        let timer_freq = match fdt.timebase_frequency() {
            Ok(frequency) => u64::from(frequency),
            Err(error) => {
                log::error!("PlatformFdt: timebase-frequency 解析失败: {}", error);
                crate::util::halt::halt("无法从 FDT 获取 RISC-V timer 频率");
            }
        };
        crate::arch::riscv64::timer::set_hw_freq(timer_freq);
    }

    log::info!("FDT: found {} nodes, {} CPUs", node_count, core_count);
    log::info!(
        "PlatformFdt: copied raw={:#x} to kernel-owned={:#x}, totalsize={:#x}",
        dtb_addr,
        fdt.storage_addr(),
        fdt.total_size()
    );
    log::info!("Memory: {} MB", mem_size / (1024 * 1024));
    log::info!(
        "FirmwareReserved: addr={:#x}, size={:#x}",
        firmware_reserved.0,
        firmware_reserved.1
    );
    log::info!("Hello SimpleKernel");
}
