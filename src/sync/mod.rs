//! Re-export from `sync` crate — 迁移中间层，保持 `use crate::sync::*` 兼容。
//!
//! `lock_stack` 已迁移到 `per-cpu` crate，通过此处保持向后兼容。
pub use sync_crate::*;

/// 向后兼容——lock_stack 已迁移到 per-cpu crate
pub mod lock_stack {
    pub use per_cpu::lock_stack::*;
}
