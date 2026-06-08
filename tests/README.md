<!-- Copyright The SimpleKernel Contributors -->

# 测试目录

独立 QEMU 系统测试——每个测试是独立二进制，启动独立 QEMU 实例，拥有干净的内核环境。
同模块的测试合并在一个包中，通过 `[[bin]]` 管理多个二进制。

## 运行

```bash
cargo xtask test --arch riscv64 --all --timeout 30                    # 全部测试
cargo xtask test --arch riscv64 --name paging-test/table --timeout 30  # 指定测试
cargo xtask test --list                                               # 列出可用测试
```

## 测试清单

| 包 | 二进制 | 类型 | 验证内容 |
|----|--------|------|---------|
| `memory-types-test` | `codec` | normal | 地址和帧/页号编解码 |
| | `align-up-overflow-panic` | should_panic | align_up 溢出检测 |
| | `align-down-canonical-panic` | should_panic | align_down_to canonical 校验 |
| | `frame-overflow-panic` | should_panic | Frame 页号范围校验 |
| | `pa-overflow-panic` | should_panic | PhysAddr 加法溢出检测 |
| | `va-canonical-panic` | should_panic | VirtAddr 规范化违反检测 |
| `paging-test` | `basic` | normal | 页大小计算、VPN 索引提取 |
| | `table` | normal | 页表 identity_map_range / update_range_flags / get_mapping |
| | `tlb-shootdown` | normal | 在线 CPU 参与 TLB shootdown 回归 |
| | `tlb-shootdown-timeout-panic` | should_panic | TLB shootdown ack 超时 fail-fast |
| | `tlb-remote-access` | normal | RISC-V 远端 CPU 在 shootdown ack 后按新权限访问目标 VA |
| | `conflict-panic` | should_panic | identity_map_range 权限冲突检测 |
| | `equal-range-panic` | should_panic | identity_map_range 空范围检测 |
| | `reversed-range-panic` | should_panic | identity_map_range 反向范围检测 |
| `sync-test` | `spinlock` | normal | SpinLock 加锁/解锁/try_lock |
| | `lockstack` | normal | 锁栈级别顺序检查 |
| | `lockstack-pop-mismatch-panic` | should_panic | 锁栈 pop 指针不匹配检测 |
| | `recursive-lock-panic` | should_panic | 同核递归加锁检测 |
| `frame-test` | `alloc` | normal | 帧分配/释放、RAII 所有权生命周期 |
| | `reserved-overlap-panic` | should_panic | init reserved/free 重叠检测 |
| | `alloc-in-hardirq-panic` | should_panic | hard IRQ 中禁止分配帧 |
| | `dealloc-in-hardirq-panic` | should_panic | hard IRQ 中禁止释放帧 |
| `memory-test` | `double-init-panic` | should_panic | memory::init 二次调用 fail-fast |
| | `fdt-multi-memory` | normal | 多段 RAM FDT 当前 fail-fast |
| | `fdt-firmware-reserved` | normal | FDT 固件 reserved-memory 解析 |
| `arch-test` | | normal | 架构启动、timer deadline、IRQ-exit 抢占、SMP online 与浮点上下文 |
| `heap-test` | | normal | 堆分配（Box、Vec、大块） |
| `device-test` | | normal | DeviceManager、VirtIO 块设备读取 |
| `fs-test` | | normal | VFS 路径解析、RamFS CRUD、多级目录 |
| `pte-test` | | normal | 页表项编解码（RISC-V + AArch64） |
| | `unaligned-paddr-panic` | should_panic | PTE 物理地址对齐校验 |
| `panic-test` | | should_panic | panic handler 正确触发 |

## 调试文件

测试二进制默认**不生成**调试文件（`.objdump`、`.readelf`、`.nm`、`.bin`），以加快构建速度。
这些文件仅在内核主二进制构建时生成（`cargo xtask build/run/debug`）。

如需为测试二进制生成调试文件，使用 `--debug-files` 标志：

```bash
cargo xtask test --arch riscv64 --name panic-test --timeout 30 --debug-files  # 指定测试
cargo xtask test --arch riscv64 --timeout 30 --debug-files                    # 全部测试
```

生成的文件位于 `target/<triple>/debug/` 目录，与测试 ELF 同名但扩展名不同。

## 添加新测试

**在已有模块包中添加（推荐）：**

1. 在对应包的 `src/` 下创建新源文件
2. 在该包的 `Cargo.toml` 中添加 `[[bin]]` 条目：

```toml
[[bin]]
name = "my-new-test"
path = "src/my_new_test.rs"
test = false
```

3. 源文件使用 `test_harness::test_main!` 宏：

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

4. xtask 自动扫描 `[[bin]]` 条目发现新测试

**创建新模块包：**

1. 创建 `tests/my-test/`，包含 `Cargo.toml`（至少一个 `[[bin]]` 条目）和对应源文件
2. 在根 `Cargo.toml` 的 `[workspace] members` 中添加路径
3. xtask 自动扫描 `tests/*/Cargo.toml` 中的 `[[bin]]` 条目发现新测试
