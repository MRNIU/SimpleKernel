//! MappedFrames drop 应 panic——验证 typestate 安全网。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(
    simplekernel::boot::InitLevel::Full,
    test_mapped_drop_panics,
    should_panic
);

/// MappedFrames 未经 unmap 直接 drop 应触发 panic。
fn test_mapped_drop_panics() {
    let allocated =
        frame_allocator::AllocatedFrames::<memory_types::Page4K>::alloc_one().expect("分配");
    let _mapped = allocated.into_mapped();
    // _mapped drop -> panic("Frames<Mapped> dropped without unmap")
}
