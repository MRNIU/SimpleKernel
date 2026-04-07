//! 堆分配器测试——验证 Box、Vec、大块分配基本功能。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_heap_box_alloc();
    log::info!("test heap_box_alloc ... ok");

    test_heap_vec_alloc();
    log::info!("test heap_vec_alloc ... ok");

    test_heap_large_alloc();
    log::info!("test heap_large_alloc ... ok");

    log::info!("heap-test: all 3 tests passed");
}

/// Box 分配——基本值和数组。
fn test_heap_box_alloc() {
    let val = Box::new(42u64);
    assert_eq!(*val, 42);
    let val2 = Box::new([0u8; 256]);
    assert_eq!(val2[0], 0);
    assert_eq!(val2[255], 0);
}

/// Vec 动态增长和索引访问。
fn test_heap_vec_alloc() {
    let mut v: Vec<u32> = Vec::new();
    for i in 0..100 {
        v.push(i);
    }
    assert_eq!(v.len(), 100);
    assert_eq!(v[99], 99);
}

/// 大块分配（4096 字节）边界检查。
fn test_heap_large_alloc() {
    let v = vec![0xAAu8; 4096];
    assert_eq!(v.len(), 4096);
    assert_eq!(v[0], 0xAA);
    assert_eq!(v[4095], 0xAA);
}
