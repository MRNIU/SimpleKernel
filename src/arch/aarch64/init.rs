// Copyright The SimpleKernel Contributors

//! AArch64 启动参数解析。

use crate::logging;

/// 从 argv[2] 解析 DTB 地址（十六进制字符串）。
///
/// AArch64 上 U-Boot bootm 将 DTB 地址作为 argv[2] 传入。
/// 解析失败时直接 panic，避免把无效启动输入静默转换为地址 0。
///
/// # Safety
/// `argc` / `argv` 必须来自 U-Boot `bootm` 传入的启动参数；当 `argc >= 3`
/// 时，`argv[2]` 必须是有效的 null 终止十六进制 C 字符串。
///
/// # Panics
/// 当启动参数数量不足、`argv` 或 `argv[2]` 为空、`argv[2]` 未终止、
/// 不是 UTF-8、不是十六进制地址或解析为地址 0 时 panic，并输出原始参数信息。
pub unsafe fn dtb_addr_from_argv(argc: i32, argv: *const *const u8) -> usize {
    if argc < 3 {
        panic!(
            "AArch64 boot 参数不足，无法读取 DTB 地址: argc={argc}, argv={argv:p}, expected_argc>=3"
        );
    }

    // SAFETY: 调用方保证 argv 满足 U-Boot bootm 参数布局。
    let addr = unsafe { parse_hex_from_argv2(argc, argv) };
    addr as usize
}

/// 从 argv[2] 解析十六进制地址
///
/// # Safety
/// `argv` 必须是 U-Boot 传入的有效指针数组。
unsafe fn parse_hex_from_argv2(argc: i32, argv: *const *const u8) -> u64 {
    if argv.is_null() {
        panic!("AArch64 boot argv 为空，无法读取 DTB 地址: argc={argc}, argv={argv:p}");
    }

    // SAFETY: U-Boot bootm 传入的 argv 至少包含 3 个元素
    let argv2 = unsafe { *argv.add(2) };
    if argv2.is_null() {
        panic!(
            "AArch64 boot argv[2] 为空，无法读取 DTB 地址: argc={argc}, argv={argv:p}, argv2={argv2:p}"
        );
    }

    let mut buf = [0u8; 32];
    let mut len = 0;
    for (i, slot) in buf.iter_mut().enumerate() {
        // SAFETY: 在 null 终止的 C 字符串范围内读取，受 buf 大小限制
        let c = unsafe { *argv2.add(i) };
        if c == 0 {
            break;
        }
        *slot = c;
        len += 1;
    }

    if len == buf.len() {
        panic!(
            "AArch64 boot argv[2] 未在最大长度内终止: argc={argc}, argv={argv:p}, argv2={argv2:p}, max_len={}, bytes={:x?}",
            buf.len(),
            &buf[..]
        );
    }
    if len == 0 {
        panic!(
            "AArch64 boot argv[2] 为空字符串，无法解析 DTB 地址: argc={argc}, argv={argv:p}, argv2={argv2:p}, len={len}"
        );
    }

    let s = core::str::from_utf8(&buf[..len]).unwrap_or_else(|error| {
        panic!(
            "AArch64 boot argv[2] 不是有效 UTF-8: argc={argc}, argv={argv:p}, argv2={argv2:p}, len={len}, error={error}, bytes={:x?}",
            &buf[..len]
        )
    });
    let hex = if let Some(stripped) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        stripped
    } else {
        s
    };
    match u64::from_str_radix(hex, 16) {
        Ok(addr) if addr != 0 => addr,
        Ok(addr) => {
            panic!(
                "AArch64 boot argv[2] 解析出无效 DTB 地址 0: argc={argc}, argv={argv:p}, argv2={argv2:p}, len={len}, raw='{s}', parsed='{hex}', parsed_addr={addr:#x}"
            );
        }
        Err(error) => {
            logging::raw_put("ERROR: 无法解析 DTB 地址: '");
            logging::raw_put(s);
            logging::raw_put("'\n");
            panic!(
                "AArch64 boot argv[2] 不是十六进制 DTB 地址: argc={argc}, argv={argv:p}, argv2={argv2:p}, len={len}, raw='{s}', parsed='{hex}', error={error}"
            );
        }
    }
}
