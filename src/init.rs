/// 早期初始化（架构无关）——解析 FDT，填充 BASIC_INFO。
///
/// 在堆和分页启用之前运行，仅依赖 logging 和栈。
pub fn early_init(dtb_addr: usize) {
    use crate::boot_info::{BASIC_INFO, BasicInfo};
    use crate::fdt::KernelFdt;
    use crate::memory::address::PhysAddr;

    let fdt = match KernelFdt::new(dtb_addr) {
        Ok(f) => f,
        Err(_) => {
            crate::logging::raw_put("FATAL: Failed to parse FDT\n");
            loop {
                core::hint::spin_loop();
            }
        }
    };

    let node_count = fdt.node_count().unwrap_or(0);
    let core_count = fdt.core_count().unwrap_or(1);

    let (mem_addr, mem_size) = match fdt.memory() {
        Ok(m) => m,
        Err(_) => {
            crate::logging::raw_put("FATAL: Failed to get memory info from FDT\n");
            loop {
                core::hint::spin_loop();
            }
        }
    };

    // SAFETY: 链接器定义的符号，地址在内核生命周期内有效
    unsafe extern "C" {
        static __executable_start: u8;
        static _end: u8;
    }
    let kernel_start = unsafe { &__executable_start as *const u8 as u64 };
    let kernel_end = unsafe { &_end as *const u8 as u64 };

    BASIC_INFO.call_once(|| BasicInfo {
        physical_memory_addr: PhysAddr::new(mem_addr as usize),
        physical_memory_size: mem_size,
        kernel_addr: PhysAddr::new(kernel_start as usize),
        kernel_size: (kernel_end - kernel_start) as usize,
        elf_addr: PhysAddr::new(kernel_start as usize),
        fdt_addr: PhysAddr::new(dtb_addr),
        core_count,
    });

    log::info!("FDT: found {} nodes, {} CPUs", node_count, core_count);
    log::info!("Memory: {} MB", mem_size / (1024 * 1024));
    log::info!("Hello SimpleKernel");
}
