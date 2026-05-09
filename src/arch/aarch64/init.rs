// Copyright The SimpleKernel Contributors

use crate::logging;

/// 从 argv[2] 解析 DTB 地址（十六进制字符串）。
///
/// AArch64 上 U-Boot bootm 将 DTB 地址作为 argv[2] 传入。
/// 解析失败时记录警告并返回 0。
///
/// # Safety
/// `argc` / `argv` 必须来自 U-Boot `bootm` 传入的启动参数；当 `argc >= 3`
/// 时，`argv[2]` 必须是有效的 null 终止十六进制 C 字符串。
pub unsafe fn dtb_addr_from_argv(argc: i32, argv: *const *const u8) -> usize {
    if argc < 3 {
        logging::raw_put("WARNING: argc < 3, cannot read DTB address\n");
        return 0;
    }

    // SAFETY: 调用方保证 argv 满足 U-Boot bootm 参数布局。
    let addr = unsafe { parse_hex_from_argv2(argv) };
    addr as usize
}

/// 从 argv[2] 解析十六进制地址
///
/// # Safety
/// `argv` 必须是 U-Boot 传入的有效指针数组。
unsafe fn parse_hex_from_argv2(argv: *const *const u8) -> u64 {
    if argv.is_null() {
        logging::raw_put("WARNING: argv is null, cannot read DTB address\n");
        return 0;
    }

    // SAFETY: U-Boot bootm 传入的 argv 至少包含 3 个元素
    let argv2 = unsafe { *argv.add(2) };
    if argv2.is_null() {
        logging::raw_put("WARNING: argv[2] is null, cannot read DTB address\n");
        return 0;
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

    let s = core::str::from_utf8(&buf[..len]).unwrap_or("");
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    match u64::from_str_radix(hex, 16) {
        Ok(addr) => addr,
        Err(_) => {
            logging::raw_put("WARNING: 无法解析 DTB 地址: '");
            logging::raw_put(s);
            logging::raw_put("'\n");
            0
        }
    }
}
