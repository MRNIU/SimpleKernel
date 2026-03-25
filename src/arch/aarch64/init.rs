use crate::logging;

/// 从 argv[2] 解析 DTB 地址（十六进制字符串）。
///
/// AArch64 上 U-Boot bootm 将 DTB 地址作为 argv[2] 传入。
/// 解析失败时记录警告并返回 0。
pub fn dtb_addr_from_argv(argv: *const *const u8) -> usize {
    // SAFETY: argv 由 _start 传入，_start 通过 boot.S 从 U-Boot 接收
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
    for i in 0..buf.len() {
        // SAFETY: 在 null 终止的 C 字符串范围内读取，受 buf 大小限制
        let c = unsafe { *argv2.add(i) };
        if c == 0 {
            break;
        }
        buf[i] = c;
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
