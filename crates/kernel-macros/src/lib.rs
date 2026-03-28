//! SimpleKernel 过程宏集合。
//!
//! 当前包含：
//! - `#[cpu_local]` — 声明 per-CPU 变量（放入 `.percpu` ELF section）

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemStatic, StaticMutability, parse_macro_input};

/// 声明一个 per-CPU 变量。
///
/// # 用法
///
/// ```ignore
/// #[cpu_local]
/// pub static MY_COUNTER: u32 = 0;
/// ```
///
/// 展开为：
/// - 裸机：变量放入 `.percpu` section，生成 `CpuLocal<T>` 包装器
/// - 宿主机：普通 static，`CpuLocal<T>` 直接指向它
#[proc_macro_attribute]
pub fn cpu_local(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStatic);

    // 校验：不能是 static mut
    if matches!(input.mutability, StaticMutability::Mut(_)) {
        return syn::Error::new_spanned(
            &input,
            "#[cpu_local] 不支持 `static mut`，可变性由 CpuLocal::get_mut() 提供",
        )
        .to_compile_error()
        .into();
    }

    let vis = &input.vis;
    let name = &input.ident;
    let ty = &input.ty;
    let expr = &input.expr;
    let attrs = &input.attrs;

    // 内部 raw 变量名（加 _RAW 后缀避免冲突）
    let raw_name = format_ident!("_PERCPU_{}_RAW", name);

    // 生成的代码
    let expanded = quote! {
        // 编译期断言：T 必须实现 Sync（CpuLocal<T> 要求 T: Sync）
        const _: () = {
            fn _assert_sync<T: Sync>() {}
            fn _check() { _assert_sync::<#ty>(); }
        };

        // 裸机：放入 .percpu section
        #[cfg(target_os = "none")]
        #[unsafe(link_section = ".percpu")]
        #[used]
        #(#attrs)*
        static #raw_name: #ty = #expr;

        // 宿主机（测试/clippy）：普通 static
        #[cfg(not(target_os = "none"))]
        #(#attrs)*
        static #raw_name: #ty = #expr;

        // 公开的 CpuLocal<T> 包装器，使用原始变量名
        #[allow(non_upper_case_globals)]
        #vis static #name: per_cpu::CpuLocal<#ty> =
            // SAFETY: _PERCPU_*_RAW 是由本宏生成的合法 .percpu section 变量
            unsafe { per_cpu::CpuLocal::__new(&#raw_name as *const #ty) };
    };

    expanded.into()
}
