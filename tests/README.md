# 测试目录

独立 QEMU 系统测试——每个测试是独立二进制，启动独立 QEMU 实例，拥有干净的内核环境。

## 运行

```bash
cargo xtask test --arch riscv64 --all          # 全部测试
cargo xtask test --arch riscv64 --name <name>  # 指定测试
cargo xtask test --list                        # 列出可用测试
```

## 测试清单

| 测试 | 类型 | 验证内容 |
|------|------|---------|
| `frame-alloc-test` | normal | 帧分配/释放、typestate 生命周期转换 |
| `frame-mapped-drop-test` | should_panic | MappedFrames 未 unmap 直接 drop 应 panic |
| `sync-spinlock-test` | normal | SpinLock 加锁/解锁/try_lock |
| `sync-recursive-lock-test` | should_panic | 同核递归加锁检测 |
| `sync-lockstack-test` | normal | 锁栈级别顺序检查 |
| `sync-lockstack-pop-mismatch-test` | should_panic | 锁栈 pop 指针不匹配检测 |
| `paging-basic-test` | normal | 页大小计算、VPN 索引提取 |
| `paging-table-test` | normal | 页表 map/unmap/get_mapping/大页/identity_map_range |
| `paging-mapping-test` | normal | MappedPages RAII 映射生命周期 |
| `paging-equal-range-panic-test` | should_panic | identity_map_range 空范围检测 |
| `paging-reversed-range-panic-test` | should_panic | identity_map_range 反向范围检测 |
| `paging-conflict-panic-test` | should_panic | identity_map_range 映射冲突检测 |
| `vma-test` | normal | VMA mmap/munmap/find_vma/register_existing |
| `heap-test` | normal | 堆分配（Box、Vec、大块） |
| `device-test` | normal | DeviceManager、VirtIO 块设备读取 |
| `fs-test` | normal | VFS 路径解析、RamFS CRUD、多级目录 |
| `panic_test` | should_panic | panic handler 正确触发 |

## 添加新测试

1. 创建 `tests/my-test/`，包含 `Cargo.toml` 和 `src/main.rs`
2. `src/main.rs` 使用 `test_harness::test_main!` 宏：

```rust
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

// 普通测试
test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    // 测试逻辑，assert 失败 = 测试失败
}
```

should_panic 测试使用第三个参数：

```rust
test_harness::test_main!(simplekernel::boot::InitLevel::Full, test_fn, should_panic);

fn test_fn() {
    // 触发预期 panic = 测试成功
}
```

3. 在根 `Cargo.toml` 的 `[workspace] members` 中添加路径
4. xtask 自动扫描 `tests/*/Cargo.toml` 发现新测试
