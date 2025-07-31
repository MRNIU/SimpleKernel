
// clang-format off

// 这段代码会被拷贝到低内存地址（通常是 0x1000）
// 因为 INIT-SIPI-SIPI 序列启动的 AP 从实模式开始执行
.section .text.smp_boot
.global smp_boot_start
.global smp_boot_end

.code32
smp_boot_start:
  cli

  # Zero data segment registers DS, ES, and SS.
  xorw    %ax,%ax
  movw $0x10,%ax
  movw    %ax,%ds
  movw    %ax,%es
  movw    %ax,%ss

smp_halt:
    hlt
    jmp smp_halt

smp_boot_end:

// clang-format on
