// Copyright The SimpleKernel Contributors

const UARTDR: usize = 0x00;
const UARTFR: usize = 0x18;
const UARTFR_TXFF: u32 = 1 << 5;

pub fn putchar(c: u8) {
    // SAFETY: super::PL011_BASE + 偏移量是 QEMU virt aarch64 上有效的 MMIO 寄存器地址
    unsafe {
        let fr = (super::PL011_BASE + UARTFR) as *const u32;
        while core::ptr::read_volatile(fr) & UARTFR_TXFF != 0 {
            core::hint::spin_loop();
        }
        let dr = (super::PL011_BASE + UARTDR) as *mut u32;
        core::ptr::write_volatile(dr, c as u32);
    }
}

pub fn puts(s: &str) {
    for byte in s.bytes() {
        putchar(byte);
    }
}
