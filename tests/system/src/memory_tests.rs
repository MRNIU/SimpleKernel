//! 内存子系统测试——验证堆分配器基本功能。

use crate::framework::TestCase;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

pub fn tests() -> &'static [TestCase] {
    &[
        TestCase {
            name: "heap_box_alloc",
            run: test_heap_box_alloc,
        },
        TestCase {
            name: "heap_vec_alloc",
            run: test_heap_vec_alloc,
        },
        TestCase {
            name: "heap_large_alloc",
            run: test_heap_large_alloc,
        },
    ]
}

fn test_heap_box_alloc() {
    let val = Box::new(42u64);
    assert_eq!(*val, 42);
    let val2 = Box::new([0u8; 256]);
    assert_eq!(val2[0], 0);
    assert_eq!(val2[255], 0);
}

fn test_heap_vec_alloc() {
    let mut v: Vec<u32> = Vec::new();
    for i in 0..100 {
        v.push(i);
    }
    assert_eq!(v.len(), 100);
    assert_eq!(v[99], 99);
}

fn test_heap_large_alloc() {
    let v = vec![0xAAu8; 4096];
    assert_eq!(v.len(), 4096);
    assert_eq!(v[0], 0xAA);
    assert_eq!(v[4095], 0xAA);
}
