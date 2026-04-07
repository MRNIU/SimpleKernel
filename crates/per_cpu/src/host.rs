//! 宿主机 mock 实现——用于 `cargo test`。
//!
//! 宿主机只有一个"CPU"，`CpuLocal::get()` 直接解引用模板指针，
//! 无需链接器符号和基地址寄存器。

use super::CpuLocal;

/// 宿主机核心 ID，固定为 0。
#[inline(always)]
pub fn current_core_id() -> usize {
    0
}

impl<T: Sync> CpuLocal<T> {
    /// 获取变量的不可变引用——宿主机只有一个 CPU，直接解引用模板指针。
    #[inline(always)]
    pub fn get(&self) -> &T {
        // SAFETY: template_ptr 指向由 #[cpu_local] 宏生成的 static 变量，
        // 宿主机单核环境下模板即数据
        unsafe { &*self.template_ptr }
    }

    /// 获取变量的可变引用。
    ///
    /// # Safety
    /// 调用方必须确保无并发访问。
    #[inline(always)]
    #[expect(clippy::mut_from_ref, reason = "per-CPU 内部可变性：宿主机测试 mock")]
    pub unsafe fn get_mut(&self) -> &mut T {
        // SAFETY: 调用方保证无并发访问
        unsafe { &mut *(self.template_ptr as *mut T) }
    }

    /// 宿主机只有一个 CPU，忽略 `target_core`，直接解引用。
    ///
    /// # Safety
    /// 调用方必须确保访问安全。
    #[inline(always)]
    pub unsafe fn get_on(&self, target_core: usize) -> &T {
        debug_assert!(
            target_core == 0,
            "宿主机只有一个 CPU，target_core={target_core}"
        );
        // SAFETY: 同 get()
        unsafe { &*self.template_ptr }
    }
}
