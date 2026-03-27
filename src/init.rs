/// 早期初始化（架构无关）——解析 FDT，填充 BASIC_INFO。
///
/// 在堆和分页启用之前运行，仅依赖 logging 和栈。
pub fn early_init(dtb_addr: usize) {
    use crate::fdt::KernelFdt;
    use boot_info::{BASIC_INFO, BasicInfo};
    use memory::address::PhysAddr;

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

    // RISC-V 的 timebase-frequency 在 FDT /cpus 节点中；AArch64 通过 CNTFRQ_EL0 读取
    let timer_freq = fdt.timebase_frequency().unwrap_or(0) as u64;

    BASIC_INFO.call_once(|| BasicInfo {
        physical_memory_addr: PhysAddr::new(mem_addr as usize),
        physical_memory_size: mem_size,
        kernel_addr: PhysAddr::new(kernel_start as usize),
        kernel_size: (kernel_end - kernel_start) as usize,
        elf_addr: PhysAddr::new(kernel_start as usize),
        fdt_addr: PhysAddr::new(dtb_addr),
        core_count,
        timer_freq,
    });

    log::info!("FDT: found {} nodes, {} CPUs", node_count, core_count);
    log::info!("Memory: {} MB", mem_size / (1024 * 1024));
    log::info!("Hello SimpleKernel");
}
