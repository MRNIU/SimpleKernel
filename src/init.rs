// Copyright The SimpleKernel Contributors

/// 早期初始化（架构无关）——解析 FDT，将信息分发到各子系统。
///
/// 在堆和分页启用之前运行，仅依赖 logging 和栈。
pub fn early_init(dtb_addr: usize) {
    use crate::CORE_COUNT;
    use crate::fdt::KernelFdt;
    use memory::{MEMORY_INFO, MemoryInfo};
    use memory_types::PhysAddr;

    let fdt = match KernelFdt::new(dtb_addr) {
        Ok(f) => f,
        Err(_) => {
            crate::util::halt::halt("无法解析 FDT");
        }
    };

    let node_count = fdt.node_count().unwrap_or(0);
    let core_count = crate::cpu_topology::init_from_fdt(&fdt);

    let (mem_addr, mem_size) = match fdt.memory() {
        Ok(m) => m,
        Err(_) => {
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

    let firmware_reserved = fdt.firmware_reserved_memory().unwrap_or_else(|_| {
        let size = kernel_start
            .checked_sub(mem_addr)
            .expect("early_init: kernel_start 小于 FDT RAM 起点");
        (mem_addr, size as usize)
    });

    MEMORY_INFO.call_once(|| MemoryInfo {
        physical_memory_addr: PhysAddr::new(mem_addr as usize),
        physical_memory_size: mem_size,
        kernel_addr: PhysAddr::new(kernel_start as usize),
        kernel_size: (kernel_end - kernel_start) as usize,
        firmware_reserved_addr: PhysAddr::new(firmware_reserved.0 as usize),
        firmware_reserved_size: firmware_reserved.1,
    });

    CORE_COUNT.call_once(|| core_count);

    crate::fdt::FDT_ADDR.call_once(|| dtb_addr);

    // RISC-V 的 timebase-frequency 在 FDT /cpus 节点中；
    // aarch64 从 CNTFRQ_EL0 寄存器直接读取，不需要此值。
    #[cfg(target_arch = "riscv64")]
    {
        let timer_freq = fdt.timebase_frequency().unwrap_or(0) as u64;
        crate::arch::riscv64::timer::set_hw_freq(timer_freq);
    }

    log::info!("FDT: found {} nodes, {} CPUs", node_count, core_count);
    log::info!("Memory: {} MB", mem_size / (1024 * 1024));
    log::info!(
        "FirmwareReserved: addr={:#x}, size={:#x}",
        firmware_reserved.0,
        firmware_reserved.1
    );
    log::info!("Hello SimpleKernel");
}
