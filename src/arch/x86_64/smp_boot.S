
// clang-format off

// 这段代码会被拷贝到低内存地址（通常是 0x1000）
// 因为 INIT-SIPI-SIPI 序列启动的 AP 从实模式开始执行
.section .text.smp_boot
.global smp_boot_start
.global smp_boot_end

.code16
smp_boot_start:
    nop
    // 关中断
    cli
    
    // 设置段寄存器
    xor %ax, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss
    
    // 设置栈指针（在实模式下使用16位寄存器）
    mov $0x7000, %sp      // 使用 0x7000 作为临时栈
    
    // 加载 GDT（使用相对地址，因为是16位模式）
    lgdt (smp_gdt_ptr_abs - smp_boot_start)
    
    // 启用保护模式
    mov %cr0, %eax
    or $0x1, %eax          // 设置 PE 位
    mov %eax, %cr0
    
    // 跳转到保护模式，必须使用长跳转来刷新流水线
    .byte 0xea             // 长跳转操作码
    .word smp_protected_mode - smp_boot_start  // 偏移
    .word 0x08             // 代码段选择子

.code32
smp_protected_mode:
    // 设置段寄存器
    mov $0x10, %ax         // 数据段选择子
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss
    
    // 设置 32 位栈指针
    mov $0x7000, %esp
    
    // 检查是否支持长模式
    mov $0x80000000, %eax
    cpuid
    cmp $0x80000001, %eax
    jb smp_halt            // 如果不支持扩展功能，停机
    
    mov $0x80000001, %eax
    cpuid
    test $(1 << 29), %edx  // 检查 LM 位
    jz smp_halt            // 如果不支持长模式，停机
    
    // 启用 PAE
    mov %cr4, %eax
    or $0x20, %eax         // 设置 PAE 位
    mov %eax, %cr4
    
    // 设置页表 - 使用现有的内核页表
    // 假设 BSP 已经设置了页表，我们重用它
    mov %cr3, %eax         // 获取当前页表基址
    mov %eax, %cr3         // 重新加载页表
    
    // 启用长模式
    mov $0xC0000080, %ecx  // IA32_EFER MSR
    rdmsr
    or $0x100, %eax        // 设置 LME 位
    wrmsr
    
    // 启用分页
    mov %cr0, %eax
    or $0x80000000, %eax   // 设置 PG 位
    mov %eax, %cr0
    
    // 跳转到 64 位长模式
    ljmp $0x18, $smp_long_mode

.code64
smp_long_mode:
    // 设置段寄存器
    mov $0x20, %ax         // 64 位数据段
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss
    
    // 设置栈指针 - 每个 CPU 需要自己的栈
    // 这里先使用一个临时栈，实际使用中应该为每个 CPU 分配独立的栈
    mov $smp_temp_stack_top, %rsp
    
    // 调用 64 位的 _boot 函数
    call _boot
    
smp_halt:
    hlt
    jmp smp_halt

// GDT 表
.align 8
smp_gdt:
    // 空描述符
    .quad 0x0000000000000000
    
    // 32 位代码段 (0x08)
    .quad 0x00CF9A000000FFFF
    
    // 32 位数据段 (0x10)
    .quad 0x00CF92000000FFFF
    
    // 64 位代码段 (0x18)
    .quad 0x00AF9A000000FFFF
    
    // 64 位数据段 (0x20)
    .quad 0x00AF92000000FFFF

smp_gdt_ptr:
    .word smp_gdt_end - smp_gdt - 1  // GDT 大小
    .long 0x8000 + (smp_gdt - smp_boot_start)   // GDT 绝对地址

smp_gdt_ptr_abs:
    .word smp_gdt_end - smp_gdt - 1  // GDT 大小
    .long 0x8000 + (smp_gdt - smp_boot_start)   // GDT 绝对地址

smp_gdt_end:

smp_boot_end:

// 临时栈空间
.section .bss.smp_boot
.align 16
smp_temp_stack:
    .space 4096         // 4KB 临时栈
smp_temp_stack_top:

// clang-format on
