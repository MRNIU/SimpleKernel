//! 内存子系统测试——验证堆分配器基本功能。

use crate::framework::TestCase;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

/// 返回内存测试组的所有测试用例
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

/// 测试 Box 分配：单个值 + 数组
fn test_heap_box_alloc() {
    let val = Box::new(42u64);
    assert_eq!(*val, 42);
    let val2 = Box::new([0u8; 256]);
    assert_eq!(val2[0], 0);
    assert_eq!(val2[255], 0);
}

/// 测试 Vec 动态增长
fn test_heap_vec_alloc() {
    let mut v: Vec<u32> = Vec::new();
    for i in 0..100 {
        v.push(i);
    }
    assert_eq!(v.len(), 100);
    assert_eq!(v[99], 99);
}

/// 测试大块分配（4096 字节）
fn test_heap_large_alloc() {
    let v = vec![0xAAu8; 4096];
    assert_eq!(v.len(), 4096);
    assert_eq!(v[0], 0xAA);
    assert_eq!(v[4095], 0xAA);
}
