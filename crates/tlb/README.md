<!-- Copyright The SimpleKernel Contributors -->

# tlb

TLB 管理——架构无关的 TLB 刷新接口 + 跨核 shootdown 回调。

## 概览

PTE 修改后必须刷新 TLB，否则 CPU 继续使用过期缓存。
本 crate 提供 RAII 守卫模式（`TlbFlushGuard`），确保：

- 页表修改完成后才 flush（守卫 drop 时触发，不会过早）
- 小范围按页 flush、大范围整体 flush（`TLB_FLUSH_THRESHOLD` 阈值切换）
- 多核环境自动触发 shootdown IPI（注册回调未接入前为 no-op）

## 调用层次

```
paging::PageTable::update_range_flags (修改 PTE)
   │
   ▼ TlbFlushGuard::new(va, count)
tlb (本 crate)
   │
   ├── arch::flush_tlb_page() (本核)
   └── TLB_SHOOTDOWN_FN (跨核 IPI)
```
