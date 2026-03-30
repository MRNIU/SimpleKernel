//! 宿主机 TLB 刷新——no-op（用于 `cargo test`）。

use super::TlbArch;

pub(crate) struct Host;

impl TlbArch for Host {
    #[inline(always)]
    fn flush_all() {}

    #[inline(always)]
    fn flush_page(_vaddr: usize) {}
}
