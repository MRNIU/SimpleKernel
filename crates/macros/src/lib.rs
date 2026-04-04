//! SimpleKernel 过程宏集合。
//!
//! 当前包含：
//! - `#[cpu_local]` — 声明 per-CPU 变量（放入 `.percpu` ELF section）

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemStatic, StaticMutability, parse_macro_input};

/// 声明一个 per-CPU 变量。
///
/// 展开为内部 raw 变量（裸机放入 `.percpu` section，宿主机为普通 static）
/// 和公开的 `CpuLocal<T>` 包装器。用户的 `#[doc]` 等属性保留在包装器上。
///
/// # 用法
///
/// ```ignore
/// /// 硬中断嵌套计数
/// #[cpu_local]
/// pub static HARDIRQ_COUNT: u32 = 0;
/// ```
#[proc_macro_attribute]
pub fn cpu_local(attr: TokenStream, item: TokenStream) -> TokenStream {
    // #[cpu_local] 不接受参数，为将来扩展（如 aligned、read_mostly）预留报错空间
    if !attr.is_empty() {
        return syn::Error::new(proc_macro2::Span::call_site(), "#[cpu_local] 不接受参数")
            .to_compile_error()
            .into();
    }

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
    let raw_name = format_ident!("_PERCPU_{}_RAW", name);

    let expanded = quote! {
        const _: () = {
            fn _assert_sync<T: Sync>() {}
            fn _check() { _assert_sync::<#ty>(); }
        };

        #[cfg(bare_metal)]
        #[unsafe(link_section = ".percpu")]
        #[used]
        static #raw_name: #ty = #expr;

        #[cfg(not(bare_metal))]
        static #raw_name: #ty = #expr;

        #(#attrs)*
        #[allow(non_upper_case_globals)]
        #vis static #name: ::per_cpu::CpuLocal<#ty> =
            // SAFETY: raw_name 是由本宏生成的 static 变量，地址在整个内核生命周期内有效
            unsafe { ::per_cpu::CpuLocal::__new(&#raw_name as *const #ty) };
    };

    expanded.into()
}
