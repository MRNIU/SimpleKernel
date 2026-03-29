# macros

SimpleKernel 过程宏集合。

## 概览

`macros` 是内核的 proc-macro crate，集中存放所有需要编译期代码生成的属性宏和派生宏。
Rust 要求 proc-macro crate 独立于普通 crate（不能在同一个 crate 中同时导出类型和过程宏），
因此本 crate 专门存放宏定义，由对应的类型 crate 再导出给用户使用。

## 当前宏

### `#[cpu_local]`

声明 per-CPU 变量——展开为 `.percpu` ELF section 中的原始变量和 `CpuLocal<T>` 包装器。

```rust
use per_cpu::cpu_local;

/// 硬中断嵌套计数
#[cpu_local]
pub static HARDIRQ_COUNT: u32 = 0;
```

展开结果：

```rust
// 编译期校验 T: Sync
const _: () = { fn _assert_sync<T: Sync>() {} fn _check() { _assert_sync::<u32>(); } };

// 裸机：放入 .percpu section（模板）
#[cfg(target_os = "none")]
#[unsafe(link_section = ".percpu")]
#[used]
static _PERCPU_HARDIRQ_COUNT_RAW: u32 = 0;

// 宿主机测试：普通 static
#[cfg(not(target_os = "none"))]
#[used]
static _PERCPU_HARDIRQ_COUNT_RAW: u32 = 0;

/// 硬中断嵌套计数
pub static HARDIRQ_COUNT: ::per_cpu::CpuLocal<u32> =
    unsafe { ::per_cpu::CpuLocal::__new(&_PERCPU_HARDIRQ_COUNT_RAW as *const u32) };
```

**约束：**

- 类型必须实现 `Sync`（编译期检查）
- 不支持 `static mut`——可变性由 `CpuLocal::get_mut()` 提供
- 不接受参数（为将来 `aligned`、`read_mostly` 等扩展预留）

**再导出路径：** 用户通过 `per_cpu::cpu_local` 使用，不直接依赖本 crate。

## 添加新宏的流程

1. 在 `src/lib.rs` 中添加 `#[proc_macro_attribute]` 或 `#[proc_macro_derive]` 函数
2. 在对应的类型 crate 中 `pub use macros::new_macro;` 再导出
3. 用户通过类型 crate 路径使用，无需感知 `macros` crate 的存在

## 注意事项

### 1. 用户不直接依赖本 crate

本 crate 不出现在其他 crate 的 `[dependencies]` 中（`per_cpu` 除外）。
用户通过 `per_cpu::cpu_local` 等再导出路径使用宏，
与 `serde` / `serde_derive` 的模式一致。

### 2. proc-macro crate 的编译特殊性

proc-macro crate 编译为宿主机动态库（不是目标架构的静态库），
因此可以使用 `std`，但不能导出类型、trait、函数等非宏项目。
