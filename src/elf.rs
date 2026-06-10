// Copyright The SimpleKernel Contributors

//! 内核 ELF 符号表解析。

use core::fmt;

use elf::ElfBytes;
use elf::endian::AnyEndian;

/// ELF 解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    /// 空地址
    InvalidAddress,
    /// 不是有效的 ELF 文件
    InvalidMagic,
    /// 32 位 ELF（仅支持 64 位）
    Unsupported32Bit,
    /// 未知的 ELF 类别
    InvalidClass,
    /// 没有 `.symtab` 段
    SymtabNotFound,
}

impl fmt::Display for ElfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for ElfError {}

/// 用于内核二进制符号表的 ELF64 解析器。
///
/// 供 panic 处理器将地址解析为函数名，
/// 以生成可读的回溯信息。
pub struct KernelElf {
    data: &'static [u8],
}

impl core::fmt::Debug for KernelElf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KernelElf")
            .field("base", &self.data.as_ptr())
            .field("len", &self.data.len())
            .finish()
    }
}

// SAFETY: KernelElf 只持有指向内核 ELF 映像的只读切片；该映像在内核生命周期内保持映射，
// 共享到其他执行上下文不会产生可变别名或数据竞争。
unsafe impl Send for KernelElf {}
// SAFETY: KernelElf 不提供内部可变性，所有解析操作只读访问同一份静态 ELF 映像。
unsafe impl Sync for KernelElf {}

impl KernelElf {
    /// 通过读取 ELF 头部，从原始地址创建一个 `KernelElf`，
    /// 并据此确定二进制总大小。
    ///
    /// # Errors
    /// - `InvalidAddress` — 空地址
    /// - `InvalidMagic` — 不是有效的 ELF 文件
    /// - `Unsupported32Bit` — 32 位 ELF（仅支持 64 位）
    /// - `InvalidClass` — 未知的 ELF 类别
    /// - `SymtabNotFound` — 没有 `.symtab` 段
    ///
    /// # Safety
    ///
    /// `elf_addr` 必须指向一个有效、完整映射的 ELF64 二进制文件，
    /// 并且在 `'static` 生命周期内保持有效。
    ///
    /// # Panics
    ///
    /// 若 ELF 头部或段头表字段与 ELF64 固定布局不一致，内部固定范围校验失败时 panic。
    pub unsafe fn new(elf_addr: u64) -> Result<Self, ElfError> {
        if elf_addr == 0 {
            return Err(ElfError::InvalidAddress);
        }

        let base = elf_addr as *const u8;

        // 读取前 64 字节（ELF64 头部）以进行校验并计算大小。
        // SAFETY: 调用者保证 `elf_addr` 指向至少覆盖 ELF64 头部的可读映射。
        let header = unsafe { core::slice::from_raw_parts(base, 64) };

        // 校验魔数
        if header[0..4] != [0x7F, b'E', b'L', b'F'] {
            return Err(ElfError::InvalidMagic);
        }

        // 校验类别。
        match header[4] {
            // ELFCLASS64（64 位）。
            2 => {}
            1 => return Err(ElfError::Unsupported32Bit),
            _ => return Err(ElfError::InvalidClass),
        }

        // 根据段头计算 ELF 总大小。
        // ELF64 头部布局：e_shoff 在 40（8 字节），e_shentsize 在 58（2 字节），
        // e_shnum 在 60（2 字节）。
        let e_shoff = u64::from_le_bytes(
            header[40..48]
                .try_into()
                .expect("ELF header e_shoff 切片转换失败: range=40..48, header_len=64"),
        ) as usize;
        let e_shentsize = u16::from_le_bytes(
            header[58..60]
                .try_into()
                .expect("ELF header e_shentsize 切片转换失败: range=58..60, header_len=64"),
        ) as usize;
        let e_shnum = u16::from_le_bytes(
            header[60..62]
                .try_into()
                .expect("ELF header e_shnum 切片转换失败: range=60..62, header_len=64"),
        ) as usize;

        let mut elf_size = e_shoff + e_shnum * e_shentsize;

        // 遍历段头以找到最大范围（.symtab 和 .strtab 等段
        // 往往位于段头表之后）。
        if e_shoff > 0 && e_shnum > 0 && e_shentsize >= 40 {
            // SAFETY: 调用者保证 ELF 映像完整映射；这里按 ELF 头部字段读取段头表范围。
            let sh_bytes =
                unsafe { core::slice::from_raw_parts(base.add(e_shoff), e_shnum * e_shentsize) };
            for i in 0..e_shnum {
                let off = i * e_shentsize;
                // Elf64_Shdr 中 sh_offset 位于第 24 字节，sh_size 位于第 32 字节。
                let sh_offset = u64::from_le_bytes(
                    sh_bytes[off + 24..off + 32]
                        .try_into()
                        .expect("ELF section header sh_offset 切片转换失败"),
                ) as usize;
                let sh_size = u64::from_le_bytes(
                    sh_bytes[off + 32..off + 40]
                        .try_into()
                        .expect("ELF section header sh_size 切片转换失败"),
                ) as usize;
                let section_end = sh_offset + sh_size;
                if section_end > elf_size {
                    elf_size = section_end;
                }
            }
        }

        // SAFETY: 调用者保证从 `base` 到计算出的 `elf_size` 范围在内核生命周期内可读。
        let data: &'static [u8] = unsafe { core::slice::from_raw_parts(base, elf_size) };

        // 使用 elf crate 进行校验，并确认存在 .symtab。
        let elf_file = ElfBytes::<AnyEndian>::minimal_parse(data).map_err(|e| {
            log::warn!(
                "ELF 解析失败 (addr={:#x}, len={}): {:?}",
                elf_addr,
                elf_size,
                e
            );
            ElfError::InvalidMagic
        })?;

        if elf_file
            .symbol_table()
            .map_err(|e| {
                log::warn!("ELF 符号表读取失败 (addr={:#x}): {:?}", elf_addr, e);
                ElfError::SymtabNotFound
            })?
            .is_none()
        {
            return Err(ElfError::SymtabNotFound);
        }

        Ok(Self { data })
    }

    #[must_use]
    pub fn elf_size(&self) -> usize {
        self.data.len()
    }

    /// 返回 `.symtab` 中符号的数量；若解析失败则返回 0。
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        let Ok(elf) = ElfBytes::<AnyEndian>::minimal_parse(self.data) else {
            return 0;
        };
        match elf.symbol_table() {
            Ok(Some((symtab, _))) => symtab.len(),
            _ => 0,
        }
    }

    /// 查找包含 `addr` 的函数名。
    ///
    /// 返回最接近且不超过 `addr` 的符号名。
    /// 若未找到匹配符号，则返回 `None`。
    #[must_use]
    pub fn lookup_symbol(&self, addr: u64) -> Option<&str> {
        let elf = ElfBytes::<AnyEndian>::minimal_parse(self.data).ok()?;
        let (symtab, strtab) = elf.symbol_table().ok()??;

        let mut best_value: u64 = 0;
        let mut best_name_idx: u32 = 0;
        let mut found_best = false;

        for sym in symtab.iter() {
            if sym.st_value == 0 {
                continue;
            }

            // 精确匹配：addr 落在 [st_value, st_value + st_size) 内
            if sym.st_size > 0 {
                let sym_end = sym.st_value.saturating_add(sym.st_size);
                if addr >= sym.st_value && addr < sym_end {
                    return strtab.get(sym.st_name as usize).ok();
                }
            } else if addr >= sym.st_value && sym.st_value > best_value {
                // 零长度符号：记录 addr 下方最近的那个
                best_value = sym.st_value;
                best_name_idx = sym.st_name;
                found_best = true;
            }
        }

        if found_best {
            return strtab.get(best_name_idx as usize).ok();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造仅包含 ELF header 的 64 字节缓冲区，用于测试前置校验逻辑。
    fn make_header_buf() -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        // ELFCLASS64（64 位）。
        buf[4] = 2;
        // ELFDATA2LSB（小端）。
        buf[5] = 1;
        // EV_CURRENT（当前版本）。
        buf[6] = 1;
        buf
    }

    /// 空地址应返回 InvalidAddress 错误。
    #[test]
    fn reject_null_address() {
        // SAFETY: 测试故意传入空地址，验证前置校验在解引用前返回错误。
        let result = unsafe { KernelElf::new(0) };
        assert_eq!(result.unwrap_err(), ElfError::InvalidAddress);
    }

    /// 错误的魔数应返回 InvalidMagic 错误。
    #[test]
    fn reject_invalid_magic() {
        let mut buf = make_header_buf();
        buf[0] = 0x00;
        // SAFETY: 栈上缓冲区至少包含 ELF 头部，测试只验证魔数拒绝路径。
        let result = unsafe { KernelElf::new(buf.as_ptr() as u64) };
        assert_eq!(result.unwrap_err(), ElfError::InvalidMagic);
    }

    /// 32 位 ELF 应返回 Unsupported32Bit 错误。
    #[test]
    fn reject_32bit_elf() {
        let mut buf = make_header_buf();
        // ELFCLASS32（32 位）。
        buf[4] = 1;
        // SAFETY: 栈上缓冲区至少包含 ELF 头部，测试只验证 32 位类别拒绝路径。
        let result = unsafe { KernelElf::new(buf.as_ptr() as u64) };
        assert_eq!(result.unwrap_err(), ElfError::Unsupported32Bit);
    }

    /// 未知的 ELF 类别应返回 InvalidClass 错误。
    #[test]
    fn reject_unknown_class() {
        let mut buf = make_header_buf();
        buf[4] = 0xFF;
        // SAFETY: 栈上缓冲区至少包含 ELF 头部，测试只验证未知类别拒绝路径。
        let result = unsafe { KernelElf::new(buf.as_ptr() as u64) };
        assert_eq!(result.unwrap_err(), ElfError::InvalidClass);
    }
}
