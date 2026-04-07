//! memory_types 地址和帧/页号编解码测试。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use alloc::format;
use memory_types::{Frame, Page, Page1G, Page2M, PhysAddr, VirtAddr};

test_harness::test_main!(simplekernel::boot::InitLevel::Memory, run_tests);

fn run_tests() {
    test_alignment_basic();
    log::info!("test alignment_basic ... ok");

    test_alignment_zero();
    log::info!("test alignment_zero ... ok");

    test_virt_addr_alignment();
    log::info!("test virt_addr_alignment ... ok");

    test_generic_alignment();
    log::info!("test generic_alignment ... ok");

    test_arithmetic_add_sub_usize();
    log::info!("test arithmetic_add_sub_usize ... ok");

    test_arithmetic_assign_ops();
    log::info!("test arithmetic_assign_ops ... ok");

    test_arithmetic_sub_returns_usize();
    log::info!("test arithmetic_sub_returns_usize ... ok");

    test_from_usize_roundtrip();
    log::info!("test from_usize_roundtrip ... ok");

    test_virt_addr_pointer_conversion();
    log::info!("test virt_addr_pointer_conversion ... ok");

    test_virt_addr_from_pointer();
    log::info!("test virt_addr_from_pointer ... ok");

    test_phys_virt_roundtrip();
    log::info!("test phys_virt_roundtrip ... ok");

    test_phys_virt_zero();
    log::info!("test phys_virt_zero ... ok");

    test_frame_addr_roundtrip();
    log::info!("test frame_addr_roundtrip ... ok");

    test_frame_from_unaligned_truncates();
    log::info!("test frame_from_unaligned_truncates ... ok");

    test_page_from_trait();
    log::info!("test page_from_trait ... ok");

    test_frame_zero();
    log::info!("test frame_zero ... ok");

    test_page_arithmetic();
    log::info!("test page_arithmetic ... ok");

    test_page_assign_ops();
    log::info!("test page_assign_ops ... ok");

    test_frame_from_usize_roundtrip();
    log::info!("test frame_from_usize_roundtrip ... ok");

    test_frame_2m_arithmetic();
    log::info!("test frame_2m_arithmetic ... ok");

    test_frame_1g_arithmetic();
    log::info!("test frame_1g_arithmetic ... ok");

    test_frame_2m_from_addr_aligns_down();
    log::info!("test frame_2m_from_addr_aligns_down ... ok");

    test_addr_display_format();
    log::info!("test addr_display_format ... ok");

    test_frame_display_format();
    log::info!("test frame_display_format ... ok");

    log::info!("memory-types-test: all tests passed");
}

/// 页对齐：已对齐地址保持不变，未对齐地址 align_down 向下取整、
/// align_up 向上取整，页末地址（0xFFF）正确归入当前页。
fn test_alignment_basic() {
    let aligned = PhysAddr::new(0x8020_0000);
    assert!(aligned.is_aligned());
    assert_eq!(aligned.page_offset(), 0);
    assert_eq!(aligned.align_down(), aligned);
    assert_eq!(aligned.align_up(), aligned);

    let unaligned = PhysAddr::new(0x8020_0001);
    assert!(!unaligned.is_aligned());
    assert_eq!(unaligned.page_offset(), 1);
    assert_eq!(unaligned.align_down(), PhysAddr::new(0x8020_0000));
    assert_eq!(unaligned.align_up(), PhysAddr::new(0x8020_1000));

    let end_of_page = PhysAddr::new(0x8020_0FFF);
    assert!(!end_of_page.is_aligned());
    assert_eq!(end_of_page.align_down(), PhysAddr::new(0x8020_0000));
    assert_eq!(end_of_page.align_up(), PhysAddr::new(0x8020_1000));
}

/// 零地址边界：地址 0 应被视为页对齐。
fn test_alignment_zero() {
    let zero = PhysAddr::new(0);
    assert!(zero.is_aligned());
    assert_eq!(zero.align_down(), zero);
    assert_eq!(zero.align_up(), zero);
}

/// VirtAddr 对齐方法（高段规范地址）。
fn test_virt_addr_alignment() {
    let v = VirtAddr::new(0xFFFF_FFC0_0000_0001);
    assert!(!v.is_aligned());
    assert_eq!(v.align_down(), VirtAddr::new(0xFFFF_FFC0_0000_0000));
    assert_eq!(v.align_up(), VirtAddr::new(0xFFFF_FFC0_0000_1000));
}

/// 通用对齐：is_aligned_to / align_down_to / align_up_to 支持任意 2 的幂。
fn test_generic_alignment() {
    let addr = PhysAddr::new(0x1_2345);

    assert!(addr.is_aligned_to(1));
    assert!(!addr.is_aligned_to(0x1_0000));
    assert!(PhysAddr::new(0x1_0000).is_aligned_to(0x1_0000));

    assert_eq!(addr.align_down_to(0x1000), PhysAddr::new(0x1_2000));
    assert_eq!(addr.align_up_to(0x1000), PhysAddr::new(0x1_3000));

    assert_eq!(addr.align_down_to(0x1_0000), PhysAddr::new(0x1_0000));
    assert_eq!(addr.align_up_to(0x1_0000), PhysAddr::new(0x2_0000));
}

/// 地址 ± usize 运算。
fn test_arithmetic_add_sub_usize() {
    let base = PhysAddr::new(0x8020_0000);
    let a = base + 0x1000;
    assert_eq!(a.as_usize(), 0x8020_1000);
    assert_eq!(a - 0x1000, base);
}

/// += 和 -= 运算符。
fn test_arithmetic_assign_ops() {
    let mut addr = PhysAddr::new(0x1000);
    addr += 0x500;
    assert_eq!(addr, PhysAddr::new(0x1500));
    addr -= 0x500;
    assert_eq!(addr, PhysAddr::new(0x1000));
}

/// 地址 - 地址 返回 usize（字节差值）。
fn test_arithmetic_sub_returns_usize() {
    let a = VirtAddr::new(0xFFFF_FFC0_0000_2000);
    let b = VirtAddr::new(0xFFFF_FFC0_0000_0000);
    let diff: usize = a - b;
    assert_eq!(diff, 0x2000);
}

/// From<usize> 和 Into<usize> 双向转换。
fn test_from_usize_roundtrip() {
    let addr: PhysAddr = 0x8020_0000usize.into();
    let val: usize = addr.into();
    assert_eq!(val, 0x8020_0000);
}

/// as_ptr / as_mut_ptr 往返。
fn test_virt_addr_pointer_conversion() {
    let addr = VirtAddr::new(0xDEAD_0000);
    assert_eq!(addr.as_ptr::<u8>() as usize, 0xDEAD_0000);
    assert_eq!(addr.as_mut_ptr::<u32>() as usize, 0xDEAD_0000);
}

/// From<*const T> 构造 VirtAddr。
fn test_virt_addr_from_pointer() {
    let canonical: usize = 0x1000;
    let ptr = canonical as *const u64;
    let addr = VirtAddr::from(ptr);
    assert_eq!(addr.as_usize(), canonical);
}

/// to_virt / to_phys 互逆。
fn test_phys_virt_roundtrip() {
    let pa = PhysAddr::new(0x8020_0000);
    let va = pa.to_virt();
    assert_eq!(
        va.as_usize(),
        pa.as_usize().wrapping_add(config::PHYS_OFFSET)
    );
    assert_eq!(va.to_phys(), pa);
}

/// 零地址的物理-虚拟转换。
fn test_phys_virt_zero() {
    let pa = PhysAddr::new(0);
    let va = pa.to_virt();
    assert_eq!(va.as_usize(), config::PHYS_OFFSET);
    assert_eq!(va.to_phys(), pa);
}

/// PhysAddr → Frame → PhysAddr 往返一致。
fn test_frame_addr_roundtrip() {
    let addr = PhysAddr::new(0x8020_3000);
    let pn = addr.page_number();
    assert_eq!(pn.as_usize(), 0x8020_3);
    assert_eq!(pn.start_addr(), addr);
}

/// 未对齐地址转页号时向下取整。
fn test_frame_from_unaligned_truncates() {
    let addr = PhysAddr::new(0x8020_3FFF);
    let pn = addr.page_number();
    assert_eq!(pn.as_usize(), 0x8020_3);
    assert_eq!(pn.start_addr(), PhysAddr::new(0x8020_3000));
}

/// From trait 双向转换。
fn test_page_from_trait() {
    let addr = VirtAddr::new(0x1_0000);
    let pn: Page = addr.into();
    assert_eq!(pn.as_usize(), 0x10);
    let back: VirtAddr = pn.into();
    assert_eq!(back, addr);
}

/// 零页号的起始地址应为 0。
fn test_frame_zero() {
    let pn: Frame = Frame::new(0);
    assert_eq!(pn.start_addr(), PhysAddr::new(0));
}

/// 页号加减偏移及两个页号相减。
fn test_page_arithmetic() {
    let pn: Page = Page::new(0x100);
    assert_eq!((pn + 3).as_usize(), 0x103);
    assert_eq!((pn - 1).as_usize(), 0xFF);
    let diff: usize = Page::new(0x105) - pn;
    assert_eq!(diff, 5);
}

/// += 和 -= 运算符。
fn test_page_assign_ops() {
    let mut pn: Page = Page::new(10);
    pn += 5;
    assert_eq!(pn.as_usize(), 15);
    pn -= 3;
    assert_eq!(pn.as_usize(), 12);
}

/// From<usize> 和 Into<usize> 双向转换。
fn test_frame_from_usize_roundtrip() {
    let pn: Frame = 42usize.into();
    let val: usize = pn.into();
    assert_eq!(val, 42);
}

/// Frame<Page2M> 算术应按 512 缩放。
fn test_frame_2m_arithmetic() {
    let f = Frame::<Page2M>::new(0);
    let f2 = f + 1;
    assert_eq!(f2.as_usize(), 512);
    assert_eq!(f2.start_addr(), PhysAddr::new(512 * 4096));

    let f3 = f + 3;
    assert_eq!(f3 - f, 3);
}

/// Frame<Page1G> 算术应按 512×512 缩放。
fn test_frame_1g_arithmetic() {
    let f = Frame::<Page1G>::new(0);
    let f2 = f + 1;
    assert_eq!(f2.as_usize(), 512 * 512);
}

/// 从 PhysAddr 转换为 Frame<Page2M> 应向下对齐。
fn test_frame_2m_from_addr_aligns_down() {
    let addr = PhysAddr::new(3 * 1024 * 1024);
    let f: Frame<Page2M> = addr.into();
    assert_eq!(f.as_usize(), 512);
    assert_eq!(f.start_addr(), PhysAddr::new(2 * 1024 * 1024));
}

/// Display 格式化应输出 16 位十六进制，高位补零。
fn test_addr_display_format() {
    let addr = PhysAddr::new(0x1000);
    assert_eq!(format!("{addr}"), "0x0000000000001000");
}

/// Frame Display 格式化。
fn test_frame_display_format() {
    let pn: Frame = Frame::new(0x42);
    assert_eq!(format!("{pn}"), "Frame<4K>(0x42)");
}
