# P9: ELF 加载器 + 进程模型

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 加载标准 ELF64 静态二进制，在独立的用户地址空间中执行。支持 `execve`、`fork`、`wait4`。

**Architecture:** 每个用户进程拥有独立 `PageTable`。内核页映射在所有进程页表中保持一致（高地址），用户页映射各自独立（低地址）。上下文切换时写入 `satp`（RISC-V）/ `TTBR0_EL1`（AArch64）。

**Depends on:** P8 完成（ecall dispatch + U-mode entry 验证通过）

**Tech Stack:** `elf` crate（已在依赖中）、现有 `PageTable` / `frame_allocator` / `AddressSpace` 基础设施

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Create | `src/user/address_space.rs` | 用户地址空间：per-process PageTable + VMA 管理 |
| Create | `src/user/elf_loader.rs` | ELF64 解析 + PT_LOAD 段映射 |
| Create | `src/user/stack_init.rs` | 用户栈初始化：argc/argv/envp/auxv 布局 |
| Create | `src/user/process.rs` | 用户进程 PCB（扩展 TCB） |
| Modify | `src/user/mod.rs` | 注册子模块，扩展 `enter_usermode` 使用 per-process 页表 |
| Modify | `src/task/tcb.rs` | TCB 添加 `user_page_table` + `user_address_space` 字段 |
| Modify | `src/task/sched.rs:schedule()` | 上下文切换时切换页表 |
| Create | `src/syscall/exec.rs` | `execve` syscall 实现 |
| Create | `src/syscall/fork.rs` | `fork` / `clone` syscall 实现 |
| Modify | `src/syscall/abi.rs` | 注册新 syscall |
| Create | `tests/umode-test/src/umode_exec.rs` | 测试：execve 加载 ELF + 运行 |
| Create | `tests/umode-test/src/umode_fork.rs` | 测试：fork + wait |

---

## Task 1: 用户地址空间（Per-Process Page Table）

**设计要点：**
- 每个用户进程拥有独立 `PageTable`（通过 `PageTable::create()` 分配根帧）
- 进程创建时，将内核页表的映射**复制**到用户页表的高地址部分
- 用户页映射使用 `user_*` flags（带 U-bit）
- `AddressSpace` 从全局唯一变为 per-process（每进程一个 VMA 管理器）

**Files:**
- Create: `src/user/address_space.rs`

- [ ] **Step 1: 定义 `UserAddressSpace`**

```rust
//! 用户地址空间——per-process 页表 + VMA 管理。
//!
//! 与内核 `AddressSpace`（全局唯一、identity mapping）不同，
//! `UserAddressSpace` 支持任意 VA→PA 映射，每个用户进程一个实例。

use alloc::collections::BTreeMap;
use memory_types::{PhysAddr, Span, VirtAddr};
use paging::{PageTable, PteFlags};

/// 用户 VMA 记录——追踪已分配区域。
pub struct UserVma {
    /// 虚拟地址范围
    pub range: Span<VirtAddr>,
    /// 权限标志
    pub flags: PteFlags,
    /// 该区域占用的物理帧（持有所有权，阻止帧被 buddy 回收）
    pub frames: alloc::vec::Vec<frame_allocator::AllocatedFrames<memory_types::Page4K>>,
}

/// 用户地址空间。
pub struct UserAddressSpace {
    /// 用户页表（Drop 时自动释放所有帧）
    page_table: PageTable,
    /// 用户 VMA 集合（以起始地址排序）
    areas: BTreeMap<VirtAddr, UserVma>,
    /// 程序 break（brk syscall 用）
    brk: VirtAddr,
    /// brk 上限
    brk_end: VirtAddr,
}

impl UserAddressSpace {
    /// 创建用户地址空间——初始化页表并复制内核映射。
    ///
    /// 内核页在所有用户页表中一致映射（无 U-bit），
    /// 确保 trap 进入内核后能正常访问内核代码和数据。
    pub fn new() -> Result<Self, paging::error::PagingError> {
        let mut user_pt = PageTable::create()?;

        // 将内核页表中的所有映射复制到用户页表
        // 只复制非 U-bit 条目（内核页）
        copy_kernel_mappings(&mut user_pt)?;

        Ok(Self {
            page_table: user_pt,
            areas: BTreeMap::new(),
            brk: VirtAddr::new(0),
            brk_end: VirtAddr::new(0),
        })
    }

    /// 返回页表根物理地址（写入 satp / TTBR）。
    pub fn page_table_root(&self) -> PhysAddr {
        self.page_table.root_paddr()
    }

    /// 映射一段虚拟地址到新分配的物理帧。
    pub fn map_region(
        &mut self,
        va_start: VirtAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Result<(), paging::error::PagingError> {
        let mut frame_list = alloc::vec::Vec::new();
        for i in 0..page_count {
            let frame = frame_allocator::AllocatedFrames::alloc_one()
                .expect("用户页帧分配失败");
            let va = va_start + i * config::PAGE_SIZE;
            let pa = frame.start_paddr();
            self.page_table.map_page(va, pa, flags)?;
            frame_list.push(frame);
        }
        // TLB flush
        tlb::flush_range(va_start.as_usize(), page_count);

        let range = Span::new(va_start, va_start + page_count * config::PAGE_SIZE);
        self.areas.insert(va_start, UserVma {
            range,
            flags,
            frames: frame_list,
        });
        Ok(())
    }

    /// 将数据复制到已映射的用户虚拟地址。
    ///
    /// 由于用户页表的 VA→PA 不是 identity mapping，
    /// 需要查询页表获取物理地址后再写入。
    pub fn write_to_user(&self, va: VirtAddr, data: &[u8]) {
        let mut offset = 0;
        while offset < data.len() {
            let page_va = (va + offset).align_down();
            let page_offset = (va + offset).as_usize() - page_va.as_usize();
            let (pa, _) = self.page_table.get_mapping(page_va)
                .expect("write_to_user: 页未映射");
            let dst = (pa.as_usize() + page_offset) as *mut u8;
            let chunk = (config::PAGE_SIZE - page_offset).min(data.len() - offset);
            // SAFETY: pa 指向已分配的物理帧，内核有完整访问权
            unsafe { core::ptr::copy_nonoverlapping(data[offset..].as_ptr(), dst, chunk) };
            offset += chunk;
        }
    }

    /// 设置 brk 范围（execve 后设置）。
    pub fn set_brk(&mut self, start: VirtAddr, end: VirtAddr) {
        self.brk = start;
        self.brk_end = end;
    }

    /// brk syscall 实现。
    pub fn brk(&mut self, new_brk: VirtAddr) -> VirtAddr {
        // ... 扩展/收缩 brk 区域，分配/释放帧
        self.brk
    }

    /// 获取页表可变引用（fork 时用于深拷贝）。
    pub fn page_table(&self) -> &PageTable {
        &self.page_table
    }

    /// 获取页表可变引用。
    pub fn page_table_mut(&mut self) -> &mut PageTable {
        &mut self.page_table
    }
}

/// 将内核页表的顶层 PTE 复制到用户页表。
///
/// 内核映射位于高地址空间，SV39 下为页表根的高 256 个条目（index 256-511）。
/// 复制顶层 PTE 即共享整棵子树——所有用户进程看到相同的内核映射。
fn copy_kernel_mappings(user_pt: &mut PageTable) -> Result<(), paging::error::PagingError> {
    // 实现：读取内核页表的根帧 PTE[256..512]，写入 user_pt 的根帧同位置。
    // 注意：仅复制 PTE 值（指针），不复制子树帧——共享同一棵子树。
    // 这是 Linux 和大多数内核的标准做法。
    todo!("copy_kernel_mappings: 读取内核页表顶层条目并写入用户页表")
}
```

- [ ] **Step 2: 实现 `copy_kernel_mappings`**

需要在 `PageTable` 上添加读取/写入根 PTE 的方法。在 `crates/paging/src/table.rs` 中添加：

```rust
/// 读取根页表第 `index` 个 PTE 的原始值。
pub fn read_root_pte(&self, index: usize) -> u64 {
    debug_assert!(index < ENTRIES_PER_TABLE);
    let table = unsafe { Table::from_paddr(self.root_paddr) };
    table.read(index).as_raw()
}

/// 写入根页表第 `index` 个 PTE。
///
/// # Safety
/// 调用者必须确保写入的 PTE 值有效且不与已有映射冲突。
pub unsafe fn write_root_pte(&mut self, index: usize, raw: u64) {
    debug_assert!(index < ENTRIES_PER_TABLE);
    let mut table = unsafe { Table::from_paddr(self.root_paddr) };
    table.write(index, PageTableEntry::from_raw(raw));
}
```

然后 `copy_kernel_mappings` 复制高半部分（RISC-V SV39: index 256..512）：

```rust
fn copy_kernel_mappings(user_pt: &mut PageTable) -> Result<(), paging::error::PagingError> {
    let kernel_pt = paging::kernel_page_table().lock();
    // SV39: 512 个条目，高半部分 index 256..512 为内核空间
    // AArch64: TTBR1_EL1 处理内核空间，无需复制（但 SimpleKernel 只用 TTBR0？待确认）
    let kernel_start_idx = paging::ENTRIES_PER_TABLE / 2;
    for i in kernel_start_idx..paging::ENTRIES_PER_TABLE {
        let pte = kernel_pt.read_root_pte(i);
        if pte != 0 {
            // SAFETY: 复制内核页表的有效 PTE 到用户页表
            unsafe { user_pt.write_root_pte(i, pte) };
        }
    }
    Ok(())
}
```

- [ ] **Step 3: 验证编译 + 单元测试**
- [ ] **Step 4: Commit**

---

## Task 2: ELF64 加载器

解析 ELF64 文件头和 `PT_LOAD` 段，映射到用户地址空间。

**Files:**
- Create: `src/user/elf_loader.rs`

- [ ] **Step 1: ELF 加载器实现**

```rust
//! ELF64 加载器——解析 PT_LOAD 段并映射到用户地址空间。
//!
//! 仅支持 ET_EXEC（静态链接），不支持 ET_DYN（需要动态链接器）。

use elf::ElfBytes;
use elf::abi::{PT_LOAD, PF_R, PF_W, PF_X, ET_EXEC};
use elf::endian::AnyEndian;
use memory_types::VirtAddr;
use paging::{PteFlags, PteFlagsOps};

use super::address_space::UserAddressSpace;

/// ELF 加载结果。
pub struct ElfLoadInfo {
    /// 程序入口地址
    pub entry: VirtAddr,
    /// Program header 表的虚拟地址（auxv AT_PHDR 需要）
    pub phdr_vaddr: VirtAddr,
    /// Program header 条目数（auxv AT_PHNUM 需要）
    pub phnum: usize,
    /// Program header 条目大小（auxv AT_PHENT 需要）
    pub phent: usize,
    /// 最高映射地址（用于设置 brk 起点）
    pub brk_start: VirtAddr,
}

/// 加载 ELF64 到用户地址空间。
///
/// # 参数
/// - `elf_data`: ELF 文件的完整内容（字节切片）
/// - `user_as`: 目标用户地址空间
///
/// # 流程
/// 1. 解析 ELF 头部，验证 magic/class/type
/// 2. 遍历 PT_LOAD 段，分配物理帧并映射到请求的虚拟地址
/// 3. 复制段数据到映射的物理帧
/// 4. 返回入口地址和元信息
pub fn load_elf(
    elf_data: &[u8],
    user_as: &mut UserAddressSpace,
) -> Result<ElfLoadInfo, ElfLoadError> {
    let elf = ElfBytes::<AnyEndian>::minimal_parse(elf_data)
        .map_err(|_| ElfLoadError::InvalidFormat)?;

    let header = elf.ehdr;

    // 仅支持 ET_EXEC（静态链接可执行文件）
    if header.e_type != ET_EXEC {
        return Err(ElfLoadError::UnsupportedType(header.e_type));
    }

    let entry = VirtAddr::new(header.e_entry as usize);
    let mut max_addr: usize = 0;
    let mut phdr_vaddr = VirtAddr::new(0);

    // 遍历 Program Header，映射 PT_LOAD 段
    let segments = elf.segments().ok_or(ElfLoadError::NoSegments)?;
    for phdr in segments.iter() {
        if phdr.p_type == elf::abi::PT_PHDR {
            phdr_vaddr = VirtAddr::new(phdr.p_vaddr as usize);
        }
        if phdr.p_type != PT_LOAD {
            continue;
        }

        let va_start = VirtAddr::new(phdr.p_vaddr as usize).align_down();
        let va_end = VirtAddr::new((phdr.p_vaddr + phdr.p_memsz) as usize).align_up();
        let page_count = (va_end - va_start) / config::PAGE_SIZE;

        // ELF flags → PTE flags
        let flags = elf_flags_to_pte(phdr.p_flags);

        // 分配帧并映射
        user_as.map_region(va_start, page_count, flags)
            .map_err(|_| ElfLoadError::MapFailed)?;

        // 复制段数据
        let file_offset = phdr.p_offset as usize;
        let file_size = phdr.p_filesz as usize;
        let page_offset = phdr.p_vaddr as usize - va_start.as_usize();
        let dst_va = va_start + page_offset;

        if file_size > 0 {
            user_as.write_to_user(dst_va, &elf_data[file_offset..file_offset + file_size]);
        }
        // BSS 段（memsz > filesz）的零初始化由 alloc_one 的零化帧保证

        if va_end.as_usize() > max_addr {
            max_addr = va_end.as_usize();
        }
    }

    // 设置 brk 起点 = 最高 PT_LOAD 段之后（页对齐）
    let brk_start = VirtAddr::new(max_addr).align_up();
    user_as.set_brk(brk_start, brk_start);

    Ok(ElfLoadInfo {
        entry,
        phdr_vaddr,
        phnum: header.e_phnum as usize,
        phent: header.e_phentsize as usize,
        brk_start,
    })
}

/// ELF p_flags → PTE flags 转换。
fn elf_flags_to_pte(p_flags: u32) -> PteFlags {
    let r = (p_flags & PF_R) != 0;
    let w = (p_flags & PF_W) != 0;
    let x = (p_flags & PF_X) != 0;
    match (r, w, x) {
        (true, true, true) => PteFlags::user_rwx(),
        (true, true, false) => PteFlags::user_rw(),
        (true, false, true) => PteFlags::user_rx(),
        (true, false, false) => PteFlags::user_ro(),
        _ => PteFlags::user_rw(), // 保守默认
    }
}

/// ELF 加载错误。
#[derive(Debug)]
pub enum ElfLoadError {
    InvalidFormat,
    UnsupportedType(u16),
    NoSegments,
    MapFailed,
}
```

- [ ] **Step 2: 验证编译**
- [ ] **Step 3: Commit**

---

## Task 3: 用户栈初始化（auxv 布局）

musl libc 的 `__libc_start_main` 期望用户栈上有标准的 argc/argv/envp/auxv 布局。

**Files:**
- Create: `src/user/stack_init.rs`

- [ ] **Step 1: 实现栈初始化**

```rust
//! 用户栈初始化——按 Linux ABI 构建 argc/argv/envp/auxv。
//!
//! 栈布局（高地址→低地址）：
//! ┌─────────────────┐ ← 栈顶（初始 sp 指向此处之下）
//! │ 字符串区域       │  argv[i] 和 envp[i] 指向的实际字节
//! │ padding (16B对齐)│
//! │ auxv[N] = {0, 0} │  AT_NULL 终止
//! │ auxv[...]        │  auxiliary vector
//! │ NULL             │  envp 终止
//! │ envp[...]        │  环境变量指针
//! │ NULL             │  argv 终止
//! │ argv[...]        │  参数指针
//! │ argc             │ ← sp
//! └─────────────────┘

use memory_types::VirtAddr;
use super::address_space::UserAddressSpace;
use super::elf_loader::ElfLoadInfo;

/// Auxiliary vector 类型（Linux ABI）。
const AT_NULL: u64 = 0;
const AT_PHDR: u64 = 3;
const AT_PHENT: u64 = 4;
const AT_PHNUM: u64 = 5;
const AT_PAGESZ: u64 = 6;
const AT_ENTRY: u64 = 9;
const AT_RANDOM: u64 = 25;

/// 用户栈大小。
pub const USER_STACK_SIZE: usize = 64 * 1024; // 64KB

/// 初始化用户栈并返回初始 sp。
///
/// 在栈顶区域布局 argc/argv/envp/auxv，返回 sp 指向 argc。
pub fn init_user_stack(
    user_as: &mut UserAddressSpace,
    stack_top: VirtAddr,
    args: &[&[u8]],        // argv 字符串列表
    env: &[&[u8]],         // envp 字符串列表
    elf_info: &ElfLoadInfo,
) -> VirtAddr {
    let mut buf = alloc::vec::Vec::new();
    let mut string_offsets_argv = alloc::vec::Vec::new();
    let mut string_offsets_envp = alloc::vec::Vec::new();

    // 1. 收集字符串（稍后写入栈顶）
    for arg in args {
        string_offsets_argv.push(buf.len());
        buf.extend_from_slice(arg);
        buf.push(0); // null terminator
    }
    for e in env {
        string_offsets_envp.push(buf.len());
        buf.extend_from_slice(e);
        buf.push(0);
    }

    // 2. 计算布局大小
    let strings_size = buf.len();
    let auxv_entries = 5; // PHDR, PHENT, PHNUM, PAGESZ, ENTRY + NULL
    let auxv_size = (auxv_entries + 1) * 16; // each entry = 2 * u64
    let pointers_size = (args.len() + 1 + env.len() + 1) * 8; // argv[] + NULL + envp[] + NULL
    let argc_size = 8;
    let total = strings_size + auxv_size + pointers_size + argc_size;
    let total_aligned = (total + 15) & !15; // 16-byte aligned

    let sp = stack_top - total_aligned;

    // 3. 构建完整栈帧到临时 buffer
    let string_base = stack_top - strings_size; // 字符串区域基地址
    let mut frame = alloc::vec![0u8; total_aligned];
    let mut pos = 0;

    // argc
    let argc = args.len() as u64;
    frame[pos..pos + 8].copy_from_slice(&argc.to_le_bytes());
    pos += 8;

    // argv pointers
    for &off in &string_offsets_argv {
        let ptr = (string_base.as_usize() + off) as u64;
        frame[pos..pos + 8].copy_from_slice(&ptr.to_le_bytes());
        pos += 8;
    }
    frame[pos..pos + 8].copy_from_slice(&0u64.to_le_bytes()); // NULL
    pos += 8;

    // envp pointers
    for &off in &string_offsets_envp {
        let ptr = (string_base.as_usize() + off) as u64;
        frame[pos..pos + 8].copy_from_slice(&ptr.to_le_bytes());
        pos += 8;
    }
    frame[pos..pos + 8].copy_from_slice(&0u64.to_le_bytes()); // NULL
    pos += 8;

    // auxv
    let auxv_pairs: [(u64, u64); 6] = [
        (AT_PHDR, elf_info.phdr_vaddr.as_usize() as u64),
        (AT_PHENT, elf_info.phent as u64),
        (AT_PHNUM, elf_info.phnum as u64),
        (AT_PAGESZ, config::PAGE_SIZE as u64),
        (AT_ENTRY, elf_info.entry.as_usize() as u64),
        (AT_NULL, 0),
    ];
    for (t, v) in auxv_pairs {
        frame[pos..pos + 8].copy_from_slice(&t.to_le_bytes());
        pos += 8;
        frame[pos..pos + 8].copy_from_slice(&v.to_le_bytes());
        pos += 8;
    }

    // strings
    let strings_start = total_aligned - strings_size;
    frame[strings_start..strings_start + strings_size].copy_from_slice(&buf);

    // 4. 写入用户地址空间
    user_as.write_to_user(sp, &frame);

    sp
}
```

- [ ] **Step 2: 验证编译**
- [ ] **Step 3: Commit**

---

## Task 4: TCB 扩展 + 上下文切换时切页表

**Files:**
- Modify: `src/task/tcb.rs` — 添加 `UserAddressSpace` 可选字段
- Modify: `src/task/sched.rs:schedule()` — 切换页表

- [ ] **Step 1: TCB 添加用户地址空间字段**

在 `TaskControlBlock` 中添加：

```rust
/// 用户地址空间（内核线程为 None，用户进程为 Some）
user_as: Option<sync::SpinLock<crate::user::address_space::UserAddressSpace>>,
```

并在 `new_kernel_thread` 中设为 `None`。添加 accessor：

```rust
/// 获取用户地址空间引用（仅用户进程有效）。
pub fn user_as(&self) -> Option<&sync::SpinLock<crate::user::address_space::UserAddressSpace>> {
    self.user_as.as_ref()
}
```

- [ ] **Step 2: schedule() 中切换页表**

在 `src/task/sched.rs` 的 `schedule()` 函数中，`switch_to` 前添加页表切换：

```rust
// 切换页表（仅当 next 是用户进程时）
if let Some(user_as) = next.user_as() {
    let root = user_as.lock().page_table_root();
    // SAFETY: root 是有效的页表物理地址
    unsafe { crate::arch::Arch::switch_page_table(root) };
} else {
    // 内核线程——切回内核页表
    let kernel_root = paging::kernel_page_table().lock().root_paddr();
    unsafe { crate::arch::Arch::switch_page_table(kernel_root) };
}
```

需在 `src/arch/mod.rs` 的 `ArchOps` trait 添加 `switch_page_table` 方法：
- RISC-V: 写 `satp` + `sfence.vma`
- AArch64: 写 `TTBR0_EL1` + `TLBI` + `DSB` + `ISB`

- [ ] **Step 3: 验证编译**
- [ ] **Step 4: Commit**

---

## Task 5: execve syscall

**Files:**
- Create: `src/syscall/exec.rs`
- Modify: `src/syscall/abi.rs`

- [ ] **Step 1: 实现 execve**

```rust
//! execve — 加载并执行 ELF 二进制。

use crate::user::{address_space::UserAddressSpace, elf_loader, stack_init};

/// sys_execve — 从文件系统加载 ELF 并执行。
///
/// P9 阶段从内核内嵌数据加载（简化），后续改为从 VFS 读取。
pub fn sys_execve(path_addr: usize, argv_addr: usize, envp_addr: usize) -> i64 {
    // 1. 读取 ELF 数据（P9: 从内嵌数据或 RamFS 读取）
    // 2. 创建新的 UserAddressSpace
    // 3. 调用 elf_loader::load_elf()
    // 4. 分配用户栈 + 调用 stack_init::init_user_stack()
    // 5. 替换当前任务的 user_as
    // 6. 调用 enter_usermode(entry, sp)
    todo!()
}
```

- [ ] **Step 2: 在 abi.rs dispatch 中注册**

```rust
221 => sys_execve(args.args[0] as usize, args.args[1] as usize, args.args[2] as usize),
```

- [ ] **Step 3: Commit**

---

## Task 6: fork syscall

**Files:**
- Create: `src/syscall/fork.rs`

- [ ] **Step 1: 实现 fork**

fork 需要：
1. 创建新的 `UserAddressSpace`，**深拷贝**父进程的页表和用户帧
2. 创建新的 TCB（复制父进程状态）
3. 设置子进程 TrapContext：a0/x0 = 0（fork 返回值）
4. 父进程返回子进程 PID

**关键设计决策：** P9 先实现完整拷贝（无 COW），后续 P10+ 可优化为 COW。

```rust
/// 深拷贝用户地址空间：分配新帧，复制内容，建立相同的 VA→PA 映射。
pub fn fork_address_space(parent: &UserAddressSpace) -> Result<UserAddressSpace, ...> {
    let mut child = UserAddressSpace::new()?;
    for (va_start, vma) in parent.areas.iter() {
        // 1. 为每个 VMA 分配新帧
        // 2. 复制父进程帧的内容到子进程帧
        // 3. 在子进程页表中建立 VA→新PA 映射
    }
    Ok(child)
}
```

- [ ] **Step 2: 在 abi.rs dispatch 中注册**

```rust
220 => sys_fork(ctx),  // clone/fork，需要传入 TrapContext 以设置子进程返回值
```

- [ ] **Step 3: 创建 QEMU 测试 `umode_fork.rs`**
- [ ] **Step 4: Commit**

---

## Task 7: 集成测试

- [ ] **Step 1:** 交叉编译一个最小的 musl 静态 hello world（`printf("hello\n"); return 0;`）
- [ ] **Step 2:** 将 ELF 嵌入测试二进制
- [ ] **Step 3:** 测试 execve 加载并运行
- [ ] **Step 4:** 测试 fork + wait 组合
- [ ] **Step 5:** Commit

---

## 注意事项

| 问题 | 说明 |
|------|------|
| **AArch64 页表分离** | AArch64 使用 TTBR0_EL1（用户）和 TTBR1_EL1（内核）。SimpleKernel 当前只用 TTBR0？需确认。如果只用 TTBR0，则内核映射需复制到用户页表。如果用 TTBR0+TTBR1，则内核映射在 TTBR1，用户映射在 TTBR0，不需要复制。 |
| **RISC-V SV39 地址空间分割** | SV39: 0x0-0x3FFFFFFFFF（低 256GB）为用户，0xFFFFFFC000000000-0xFFFFFFFFFFFFFFFF（高 256GB）为内核。当前内核在低地址（identity mapping 0x80000000）。若要用高低分割，需要重新设计内核链接地址。P9 可暂时不分割——所有映射在同一页表中。 |
| **帧零化** | `AllocatedFrames` 分配的帧是否已零化？BSS 段依赖此。需确认或在 ELF loader 中显式零化。 |
| **TLB 一致性** | 页表切换后需要 TLB 全局刷新。RISC-V: `sfence.vma`；AArch64: `TLBI vmalle1` + barrier。 |
