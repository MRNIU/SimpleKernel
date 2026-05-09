// Copyright The SimpleKernel Contributors

//! 抢占控制——re-export `interrupt_state` 的抢占原语。
//!
//! 抢占状态（`PREEMPT_DISABLE_COUNT`、`NEED_RESCHED`）及其操作函数
//! 定义在 `interrupt_state` crate 中，供 `sync` crate 直接访问。
//! 本模块为内核代码提供便捷的 re-export。

pub use interrupt_state::NEED_RESCHED;
pub use interrupt_state::check_and_clear_need_resched;
pub use interrupt_state::preemptible;
pub use interrupt_state::set_need_resched_on;
