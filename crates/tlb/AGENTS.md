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
   ├── arch_primitives::flush_tlb_page() (本核)
   └── TLB_SHOOTDOWN_FN (跨核 IPI)
```

## 验证入口

- 文档-only 变更：`git diff --check`。
- flush guard、threshold 或 shootdown callback 变更：`docker exec -w /workspace simplekernel-devcontainer cargo clippy -p tlb -- -D warnings`。
- 跨核 shootdown 或权限更新可见性变化：`docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --name paging-test/tlb-shootdown --timeout 30`，并按影响面补跑 `paging-test/tlb-remote-access`。

## 不要假设

- 不要把未注册 shootdown callback 的 no-op 状态写成多核完整保证。
- 不要绕过 `TlbFlushGuard` 直接修改 PTE 后忘记刷新。
