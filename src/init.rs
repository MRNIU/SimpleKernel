/// 早期初始化（架构无关）——解析 FDT，将信息分发到各子系统。
///
/// 在堆和分页启用之前运行，仅依赖 logging 和栈。
pub fn early_init(dtb_addr: usize) {
    use crate::fdt::KernelFdt;
    use address::PhysAddr;
    use memory::{MEMORY_INFO, MemoryInfo};
    use per_cpu::CORE_COUNT;

    let fdt = match KernelFdt::new(dtb_addr) {
        Ok(f) => f,
        Err(_) => {
            crate::util::halt::halt("无法解析 FDT");
        }
    };

    let node_count = fdt.node_count().unwrap_or(0);
    let core_count = fdt.core_count().unwrap_or(1);

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

    // ── 分发到各子系统 ──

    MEMORY_INFO.call_once(|| MemoryInfo {
        physical_memory_addr: PhysAddr::new(mem_addr as usize),
        physical_memory_size: mem_size,
        kernel_addr: PhysAddr::new(kernel_start as usize),
        kernel_size: (kernel_end - kernel_start) as usize,
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
    log::info!("Hello SimpleKernel");
}
