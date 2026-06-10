// Copyright The SimpleKernel Contributors

//! 架构无关早期初始化入口。

/// 解析 FDT，并在堆和分页启用前将平台信息分发到各子系统。
pub fn early_init(dtb_addr: usize) {
    use crate::CORE_COUNT;
    use memory::{MEMORY_INFO, MemoryInfo};
    use memory_types::PhysAddr;

    // SAFETY: dtb_addr 来自架构启动入口；bootloader 契约保证它在 early_init 期间可读。
    let fdt = match unsafe { platform_fdt::init_from_raw(dtb_addr) } {
        Ok(fdt) => fdt,
        Err(error) => {
            panic!(
                "PlatformFdt: 无法初始化内核自有 DTB 副本: dtb_addr={dtb_addr:#x}, error={error}"
            );
        }
    };

    let node_count = match fdt.node_count() {
        Ok(count) => count,
        Err(error) => {
            panic!(
                "PlatformFdt: FDT 节点计数失败: raw_dtb={dtb_addr:#x}, storage={:#x}, totalsize={:#x}, error={error}",
                fdt.storage_addr(),
                fdt.total_size()
            );
        }
    };
    let core_count = crate::cpu_topology::init_from_fdt(fdt);

    let (mem_addr, mem_size) = match fdt.memory() {
        Ok(m) => m,
        Err(error) => {
            panic!(
                "PlatformFdt: FDT memory 解析失败: raw_dtb={dtb_addr:#x}, storage={:#x}, totalsize={:#x}, error={error}",
                fdt.storage_addr(),
                fdt.total_size()
            );
        }
    };

    // SAFETY: 链接器定义的符号，地址在内核生命周期内有效
    unsafe extern "C" {
        static __executable_start: u8;
        static _end: u8;
    }
    let kernel_start = unsafe { &__executable_start as *const u8 as u64 };
    let kernel_end = unsafe { &_end as *const u8 as u64 };
    assert!(
        kernel_end >= kernel_start,
        "early_init: kernel_end 小于 kernel_start: kernel_start={kernel_start:#x}, kernel_end={kernel_end:#x}"
    );

    let firmware_reserved = match fdt.firmware_reserved_memory() {
        Ok(region) => region,
        Err(error) => {
            panic!(
                "PlatformFdt: firmware reserved-memory 解析失败: raw_dtb={dtb_addr:#x}, storage={:#x}, totalsize={:#x}, mem_addr={mem_addr:#x}, mem_size={mem_size:#x}, kernel_start={kernel_start:#x}, kernel_end={kernel_end:#x}, error={error}",
                fdt.storage_addr(),
                fdt.total_size()
            );
        }
    };

    if let Some(existing) = MEMORY_INFO.get() {
        panic!(
            "MemoryInfo: 重复初始化: existing_mem={}+{:#x}, existing_kernel={}+{:#x}, new_mem={mem_addr:#x}+{mem_size:#x}, new_kernel={kernel_start:#x}+{:#x}, dtb_addr={dtb_addr:#x}",
            existing.physical_memory_addr,
            existing.physical_memory_size,
            existing.kernel_addr,
            existing.kernel_size,
            kernel_end - kernel_start
        );
    }
    MEMORY_INFO.call_once(|| MemoryInfo {
        physical_memory_addr: PhysAddr::new(mem_addr as usize),
        physical_memory_size: mem_size,
        kernel_addr: PhysAddr::new(kernel_start as usize),
        kernel_size: (kernel_end - kernel_start) as usize,
        firmware_reserved_addr: PhysAddr::new(firmware_reserved.0 as usize),
        firmware_reserved_size: firmware_reserved.1,
    });

    if let Some(existing) = CORE_COUNT.get() {
        panic!(
            "CORE_COUNT: 重复初始化: existing={}, new={}, MAX_CORE_COUNT={}, dtb_addr={dtb_addr:#x}",
            existing,
            core_count,
            config::MAX_CORE_COUNT
        );
    }
    CORE_COUNT.call_once(|| core_count);

    // RISC-V 的 timebase-frequency 在 FDT /cpus 节点中；
    // aarch64 从 CNTFRQ_EL0 寄存器直接读取，不需要此值。
    #[cfg(target_arch = "riscv64")]
    {
        let timer_freq = match fdt.timebase_frequency() {
            Ok(frequency) => u64::from(frequency),
            Err(error) => {
                panic!(
                    "PlatformFdt: timebase-frequency 解析失败: raw_dtb={dtb_addr:#x}, storage={:#x}, totalsize={:#x}, error={error}",
                    fdt.storage_addr(),
                    fdt.total_size()
                );
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
