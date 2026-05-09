<!-- Copyright The SimpleKernel Contributors -->

# DMA API Wrapper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `crates/dma` abstraction that wraps `dma-api` for SimpleKernel, then route the current VirtIO HAL DMA path through that crate without changing QEMU behavior.

**Architecture:** `crates/dma` is the only direct dependency point for `dma-api`. It exposes SimpleKernel names (`DmaDevice`, `DmaBuffer<T>`, `DmaArray<T>`, `StreamingMapping<T>`) and implements a QEMU identity-mapped backend for the existing VirtIO path. `src/device/hal.rs` stays as the `virtio-drivers::Hal` adapter and no longer owns frame allocation or DMA tracking.

**Tech Stack:** Rust `no_std`, `dma-api 0.7.2`, `zerocopy 0.8`, SimpleKernel `frame_allocator`, `memory_types`, `sync`, `virtio-drivers`, Dev Container validation.

---

## File Structure

- Create `crates/dma/Cargo.toml`: workspace crate metadata and dependencies.
- Create `crates/dma/README.md`: crate boundary, dependency rationale, QEMU-only caveat.
- Create `crates/dma/src/lib.rs`: public module surface and re-exports.
- Create `crates/dma/src/direction.rs`: SimpleKernel `DmaDirection` wrapper and conversion to `dma_api::DmaDirection`.
- Create `crates/dma/src/error.rs`: SimpleKernel-owned `DmaError` variants plus raw-region errors, with crate-private boundary conversions from `dma_api::DmaError`.
- Create `crates/dma/src/qemu.rs`: `QemuIdentityDmaOp` and raw page helpers for the VirtIO HAL.
- Create `crates/dma/src/device.rs`: `DmaDevice`, `DmaBuffer<T>`, `DmaArray<T>`, `StreamingMapping<T>` wrappers.
- Modify `Cargo.toml`: add `crates/dma` to workspace, add `dma-api` to workspace dependencies, add `dma` dependency to root crate.
- Modify `src/device/hal.rs`: delegate DMA operations to `crates/dma`.
- Create `docs/adr/014-qemu-virtio-dma-api-wrapper.md`: proposed ADR for using a wrapper around `dma-api`.
- Modify `docs/adr/README.md`: add ADR-014 row.
- Modify `docs/audit/audit-progress.md`: update current DMA status and validation notes.

---

### Task 1: Add Dependency And Empty Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/dma/Cargo.toml`
- Create: `crates/dma/src/lib.rs`
- Create: `crates/dma/README.md`

- [ ] **Step 1: Add workspace entries**

In root `Cargo.toml`, add the new workspace member after `crates/memory`:

```toml
    "crates/memory",
    "crates/dma",
    "tests/test_harness",
```

In `[workspace.dependencies]`, add `dma-api` after `virtio-drivers`:

```toml
# DMA 抽象封装在 crates/dma 内，避免 dma_api::* 扩散到设备层。
dma-api = "0.7.2"
```

In the root `[dependencies]`, add the internal crate after `frame_allocator`:

```toml
dma = { path = "crates/dma" }
```

- [ ] **Step 2: Create `crates/dma/Cargo.toml`**

```toml
[package]
name = "dma"
version.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "SimpleKernel DMA abstraction wrapping dma-api"
edition.workspace = true

[dependencies]
config = { path = "../config" }
frame_allocator = { path = "../frame_allocator" }
memory_types = { path = "../memory_types" }
sync_crate = { path = "../sync", package = "sync" }

dma-api.workspace = true
log.workspace = true
zerocopy.workspace = true

[lints]
workspace = true
```

- [ ] **Step 3: Create initial `crates/dma/src/lib.rs`**

```rust
//! DMA 抽象层。
//!
//! 本 crate 是 SimpleKernel 对 `dma-api` 的唯一直接封装点。上层模块使用
//! `DmaDevice` / `DmaBuffer` / `DmaArray` / `StreamingMapping`，不要直接依赖
//! `dma_api::*` 类型。
//!
//! 当前后端是 QEMU VirtIO identity mapping 实现，不承诺 non-coherent 真机 DMA
//! cache/PTE 语义正确性。

#![no_std]
```

- [ ] **Step 4: Create `crates/dma/README.md`**

```markdown
# dma

`crates/dma` 是 SimpleKernel 的 DMA 抽象边界。它封装 `dma-api`，避免
`dma_api::*` 类型直接扩散到设备层和未来驱动层。

当前实现只覆盖 QEMU VirtIO + SAS identity mapping：

- coherent allocation 使用 `frame_allocator::AllocatedFrames` 分配连续物理页。
- streaming mapping 使用 VA -> PA 转换。
- cache clean / invalidate 在当前 QEMU 阶段不提供真实硬件语义。

后续真机 non-coherent DMA 支持需要单独设计 cache maintenance、PTE 属性和设备
DMA capability。
```

- [ ] **Step 5: Check empty crate**

Run inside the project Dev Container:

```bash
devcontainer exec --workspace-folder . cargo check -p dma
```

Expected: the `dma` crate compiles and the command exits 0.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/dma
git commit --signoff -m "feat(dma): add dma wrapper crate"
```

---

### Task 2: Add Direction And Error Wrappers

**Files:**
- Create: `crates/dma/src/direction.rs`
- Create: `crates/dma/src/error.rs`
- Modify: `crates/dma/src/lib.rs`

- [ ] **Step 1: Add `DmaDirection` wrapper**

Create `crates/dma/src/direction.rs`:

```rust
//! DMA 方向类型。

/// DMA 传输方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DmaDirection {
    /// CPU 写入，设备读取。
    ToDevice,
    /// 设备写入，CPU 读取。
    FromDevice,
    /// CPU 和设备都可能读写。
    Bidirectional,
}

impl From<DmaDirection> for dma_api::DmaDirection {
    fn from(direction: DmaDirection) -> Self {
        match direction {
            DmaDirection::ToDevice => dma_api::DmaDirection::ToDevice,
            DmaDirection::FromDevice => dma_api::DmaDirection::FromDevice,
            DmaDirection::Bidirectional => dma_api::DmaDirection::Bidirectional,
        }
    }
}
```

- [ ] **Step 2: Add `DmaError` wrapper**

Create `crates/dma/src/error.rs`:

```rust
//! DMA 错误类型。

use core::{alloc::LayoutError, fmt};

/// DMA 操作结果。
pub type DmaResult<T> = Result<T, DmaError>;

/// SimpleKernel DMA 错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmaError {
    /// DMA 内存不足。
    NoMemory,
    /// DMA 内存布局无效。
    InvalidLayout,
    /// DMA 地址不满足设备 mask。
    DmaMaskNotMatch { addr: u64, mask: u64 },
    /// DMA 地址不满足对齐要求。
    AlignMismatch { required: usize, address: u64 },
    /// DMA 指针为空。
    NullPointer,
    /// DMA buffer 大小为 0。
    ZeroSizedBuffer,
    /// 页数为 0，无法分配或释放 DMA 区域。
    ZeroPages,
    /// 释放不存在的 raw DMA 区域。
    UnknownRawRegion { paddr: u64 },
    /// 释放 raw DMA 区域时页数和分配记录不一致。
    RawRegionPageMismatch {
        paddr: u64,
        expected_pages: usize,
        actual_pages: usize,
    },
    /// 释放 raw DMA 区域时虚拟地址和分配记录不一致。
    RawRegionVirtualAddressMismatch {
        paddr: u64,
        expected_vaddr: usize,
        actual_vaddr: usize,
    },
    /// DMA 虚拟地址为空。
    NullVirtualAddress { paddr: u64 },
}

impl From<LayoutError> for DmaError {
    fn from(_error: LayoutError) -> Self {
        DmaError::InvalidLayout
    }
}

impl DmaError {
    #[expect(dead_code, reason = "后续 qemu/device 模块会在 crate 内转换 dma-api 错误")]
    pub(crate) fn from_api(error: dma_api::DmaError) -> Self {
        match error {
            dma_api::DmaError::NoMemory => DmaError::NoMemory,
            dma_api::DmaError::LayoutError(_error) => DmaError::InvalidLayout,
            dma_api::DmaError::DmaMaskNotMatch { addr, mask } => DmaError::DmaMaskNotMatch {
                addr: addr.as_u64(),
                mask,
            },
            dma_api::DmaError::AlignMismatch { required, address } => DmaError::AlignMismatch {
                required,
                address: address.as_u64(),
            },
            dma_api::DmaError::NullPointer => DmaError::NullPointer,
            dma_api::DmaError::ZeroSizedBuffer => DmaError::ZeroSizedBuffer,
        }
    }
}

impl fmt::Display for DmaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DmaError::NoMemory => write!(f, "DMA 内存不足"),
            DmaError::InvalidLayout => write!(f, "DMA 内存布局无效"),
            DmaError::DmaMaskNotMatch { addr, mask } => {
                write!(f, "DMA 地址不满足设备 mask: addr={addr:#x}, mask={mask:#x}")
            }
            DmaError::AlignMismatch { required, address } => write!(
                f,
                "DMA 地址不满足对齐要求: required={required}, address={address:#x}",
            ),
            DmaError::NullPointer => write!(f, "DMA 指针为空"),
            DmaError::ZeroSizedBuffer => write!(f, "DMA buffer 大小为 0"),
            DmaError::ZeroPages => write!(f, "DMA 页数不能为 0"),
            DmaError::UnknownRawRegion { paddr } => {
                write!(f, "未找到 raw DMA 区域: paddr={paddr:#x}")
            }
            DmaError::RawRegionPageMismatch {
                paddr,
                expected_pages,
                actual_pages,
            } => write!(
                f,
                "raw DMA 区域页数不匹配: paddr={paddr:#x}, expected={expected_pages}, actual={actual_pages}",
            ),
            DmaError::RawRegionVirtualAddressMismatch {
                paddr,
                expected_vaddr,
                actual_vaddr,
            } => write!(
                f,
                "raw DMA 区域虚拟地址不匹配: paddr={paddr:#x}, expected_vaddr={expected_vaddr:#x}, actual_vaddr={actual_vaddr:#x}",
            ),
            DmaError::NullVirtualAddress { paddr } => {
                write!(f, "DMA 虚拟地址为空: paddr={paddr:#x}")
            }
        }
    }
}

impl core::error::Error for DmaError {}
```

- [ ] **Step 3: Export wrapper modules**

Replace `crates/dma/src/lib.rs` with:

```rust
//! DMA 抽象层。
//!
//! 本 crate 是 SimpleKernel 对 `dma-api` 的唯一直接封装点。上层模块使用
//! `DmaDevice` / `DmaBuffer` / `DmaArray` / `StreamingMapping`，不要直接依赖
//! `dma_api::*` 类型。
//!
//! 当前后端是 QEMU VirtIO identity mapping 实现，不承诺 non-coherent 真机 DMA
//! cache/PTE 语义正确性。

#![no_std]

pub mod direction;
pub mod error;

pub use direction::DmaDirection;
pub use error::{DmaError, DmaResult};
```

- [ ] **Step 4: Check the crate**

Run:

```bash
devcontainer exec --workspace-folder . cargo check -p dma
```

Expected: the `dma` crate compiles and the command exits 0.

- [ ] **Step 5: Commit**

```bash
git add crates/dma/src/lib.rs crates/dma/src/direction.rs crates/dma/src/error.rs
git commit --signoff -m "feat(dma): add direction and error wrappers"
```

---

### Task 3: Implement QEMU Identity DMA Backend

**Files:**
- Create: `crates/dma/src/qemu.rs`
- Modify: `crates/dma/src/error.rs`
- Modify: `crates/dma/src/lib.rs`

- [ ] **Step 1: Implement `QemuIdentityDmaOp` and raw helpers**

Create `crates/dma/src/qemu.rs`:

```rust
//! QEMU VirtIO identity-mapped DMA backend.

extern crate alloc;

use alloc::collections::BTreeMap;
use core::alloc::Layout;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

use dma_api::{DmaAddr, DmaHandle, DmaMapHandle, DmaOp};
use frame_allocator::AllocatedFrames;
use memory_types::VirtAddr;
use sync_crate::SpinLock;

use crate::{DmaDirection, DmaError, DmaResult};

/// QEMU VirtIO identity-mapped DMA 操作实现。
pub struct QemuIdentityDmaOp;

pub(crate) static QEMU_IDENTITY_DMA_OP: QemuIdentityDmaOp = QemuIdentityDmaOp;

static DMA_TRACKER: SpinLock<BTreeMap<u64, AllocatedFrames>> =
    SpinLock::new(BTreeMap::new(), "dma_tracker", sync_crate::lock_level::DMA);

impl DmaOp for QemuIdentityDmaOp {
    fn page_size(&self) -> usize {
        config::PAGE_SIZE
    }

    unsafe fn map_single(
        &self,
        _dma_mask: u64,
        addr: NonNull<u8>,
        size: NonZeroUsize,
        align: usize,
        _direction: dma_api::DmaDirection,
    ) -> Result<DmaMapHandle, dma_api::DmaError> {
        let vaddr = VirtAddr::new(addr.as_ptr() as usize);
        let paddr = vaddr.to_phys().as_usize() as u64;
        let layout = Layout::from_size_align(size.get(), align)?;

        // SAFETY: 当前内核使用 SAS identity mapping；`addr` 指向调用方提供的连续
        // buffer，`paddr` 是同一区域的设备可见地址，layout 来自调用方 size/align。
        Ok(unsafe { DmaMapHandle::new(addr, DmaAddr::from(paddr), layout, None) })
    }

    unsafe fn unmap_single(&self, _handle: DmaMapHandle) {}

    unsafe fn alloc_coherent(&self, _dma_mask: u64, layout: Layout) -> Option<DmaHandle> {
        let pages = layout.size().div_ceil(config::PAGE_SIZE).max(1);
        let frames = AllocatedFrames::alloc(pages).ok()?;
        let paddr = frames.start_paddr();
        let vaddr = paddr.to_virt();
        let ptr = NonNull::new(vaddr.as_mut_ptr::<u8>())?;

        // SAFETY: identity mapping 下 PA.to_virt() 有效；帧刚分配，尚无其他引用。
        unsafe {
            core::ptr::write_bytes(ptr.as_ptr(), 0, pages * config::PAGE_SIZE);
        }

        DMA_TRACKER.lock().insert(paddr.as_usize() as u64, frames);

        // SAFETY: `ptr` 指向刚分配并清零的连续 DMA 内存；DMA 地址与 identity
        // mapping 下的物理地址一致；layout 由调用方 `dma-api` 校验。
        Some(unsafe { DmaHandle::new(ptr, DmaAddr::from(paddr.as_usize() as u64), layout) })
    }

    unsafe fn dealloc_coherent(&self, handle: DmaHandle) {
        let paddr = handle.dma_addr().as_u64();
        if DMA_TRACKER.lock().remove(&paddr).is_none() {
            log::error!("dma dealloc: 未找到 paddr={paddr:#x} 的 DMA 分配记录");
        }
    }
}

/// 分配页级 raw coherent DMA 区域，供 `virtio-drivers::Hal::dma_alloc` 使用。
pub fn raw_alloc_pages(pages: usize, _direction: DmaDirection) -> DmaResult<(u64, NonNull<u8>)> {
    if pages == 0 {
        return Err(DmaError::ZeroPages);
    }

    let layout = Layout::from_size_align(pages * config::PAGE_SIZE, config::PAGE_SIZE)
        .map_err(DmaError::from)?;
    let handle = unsafe { QEMU_IDENTITY_DMA_OP.alloc_coherent(u64::MAX, layout) }
        .ok_or(DmaError::NoMemory)?;

    Ok((handle.dma_addr().as_u64(), handle.as_ptr()))
}

/// 释放页级 raw coherent DMA 区域。
pub fn raw_dealloc_pages(paddr: u64, vaddr: NonNull<u8>, pages: usize) -> DmaResult<()> {
    if pages == 0 {
        return Err(DmaError::ZeroPages);
    }

    let mut tracker = DMA_TRACKER.lock();
    let frames = tracker
        .remove(&paddr)
        .ok_or(DmaError::UnknownRawRegion { paddr })?;
    let actual_pages = frames.page_count();
    if actual_pages != pages {
        tracker.insert(paddr, frames);
        return Err(DmaError::RawRegionPageMismatch {
            paddr,
            expected_pages: pages,
            actual_pages,
        });
    }
    let expected_vaddr = frames.start_paddr().to_virt().as_usize();
    let actual_vaddr = vaddr.as_ptr() as usize;
    if expected_vaddr != actual_vaddr {
        tracker.insert(paddr, frames);
        return Err(DmaError::RawRegionVirtualAddressMismatch {
            paddr,
            expected_vaddr,
            actual_vaddr,
        });
    }

    Ok(())
}

/// 映射已有 buffer 为设备可见 DMA 地址。
///
/// # Safety
///
/// `buffer` 必须在 DMA 共享期间指向有效的连续内存区域，并且不能以违反
/// VirtIO HAL 契约的方式被并发修改。
pub unsafe fn raw_map_single(buffer: NonNull<[u8]>, direction: DmaDirection) -> DmaResult<u64> {
    // SAFETY: `virtio-drivers::Hal::share` 的调用方保证 buffer 在共享期间有效。
    let slice = unsafe { buffer.as_ref() };
    let size = NonZeroUsize::new(slice.len()).ok_or(DmaError::ZeroSizedBuffer)?;
    let ptr = NonNull::new(slice.as_ptr() as *mut u8).ok_or(DmaError::NullVirtualAddress {
        paddr: 0,
    })?;

    let handle = unsafe { QEMU_IDENTITY_DMA_OP.map_single(u64::MAX, ptr, size, 1, direction.into()) }
        .map_err(DmaError::from_api)?;
    Ok(handle.dma_addr().as_u64())
}

/// 解除已有 buffer 的 streaming DMA 映射。
pub fn raw_unmap_single(_paddr: u64, _buffer: NonNull<[u8]>, _direction: DmaDirection) {}
```

- [ ] **Step 2: Remove temporary `from_api` dead-code expectation**

After `crates/dma/src/qemu.rs` calls `DmaError::from_api` for the first time, remove this temporary line from `crates/dma/src/error.rs`:

```rust
#[expect(dead_code, reason = "后续 qemu/device 模块会在 crate 内转换 dma-api 错误")]
```

- [ ] **Step 3: Export QEMU helpers**

Replace `crates/dma/src/lib.rs` with:

```rust
//! DMA 抽象层。
//!
//! 本 crate 是 SimpleKernel 对 `dma-api` 的唯一直接封装点。上层模块使用
//! `DmaDevice` / `DmaBuffer` / `DmaArray` / `StreamingMapping`，不要直接依赖
//! `dma_api::*` 类型。
//!
//! 当前后端是 QEMU VirtIO identity mapping 实现，不承诺 non-coherent 真机 DMA
//! cache/PTE 语义正确性。

#![no_std]

pub mod direction;
pub mod error;
mod qemu;

pub use direction::DmaDirection;
pub use error::{DmaError, DmaResult};
pub use qemu::{raw_alloc_pages, raw_dealloc_pages, raw_map_single, raw_unmap_single};
```

- [ ] **Step 4: Check the crate**

Run:

```bash
devcontainer exec --workspace-folder . cargo check -p dma
```

Expected: the `dma` crate compiles and the command exits 0.

- [ ] **Step 5: Commit**

```bash
git add crates/dma/src/lib.rs crates/dma/src/error.rs crates/dma/src/qemu.rs
git commit --signoff -m "feat(dma): implement qemu identity backend"
```

---

### Task 4: Wrap Typed DMA Containers

**Files:**
- Create: `crates/dma/src/device.rs`
- Modify: `crates/dma/src/lib.rs`

- [ ] **Step 1: Add typed wrappers**

Create `crates/dma/src/device.rs`:

```rust
//! Typed DMA 容器封装。

use core::ptr::NonNull;

use crate::{DmaDirection, DmaError, DmaResult};

/// 可安全放入 typed DMA buffer 的 Plain Old Data 类型。
///
/// `FromBytes` 保证任意设备写入位模式可构造为 `T`，`IntoBytes` 保证 CPU 写入
/// 可以按字节交给设备读取，`Copy` 避免从 DMA 内存读取时产生所有权搬移语义。
///
/// 面向设备的 descriptor 类型仍应使用 `#[repr(C)]` 和固定宽度整数字段；
/// `DmaValue` 不编码设备 ABI 或 endian 语义。
pub trait DmaValue: zerocopy::FromBytes + zerocopy::IntoBytes + Copy {}

impl<T> DmaValue for T where T: zerocopy::FromBytes + zerocopy::IntoBytes + Copy {}

/// 设备 DMA 入口。
#[derive(Clone)]
pub struct DmaDevice {
    inner: dma_api::DeviceDma,
}

impl DmaDevice {
    /// 创建 QEMU identity-mapped DMA 设备入口。
    pub fn qemu_identity(dma_mask: u64) -> Self {
        Self {
            inner: dma_api::DeviceDma::new(dma_mask, &crate::qemu::QEMU_IDENTITY_DMA_OP),
        }
    }

    /// 返回设备 DMA mask。
    pub fn dma_mask(&self) -> u64 {
        self.inner.dma_mask()
    }

    /// 分配单个 typed coherent DMA buffer。
    pub fn buffer_zeroed<T: DmaValue>(
        &self,
        align: usize,
        direction: DmaDirection,
    ) -> DmaResult<DmaBuffer<T>> {
        self.inner
            .box_zero_with_align::<T>(align, direction.into())
            .map(DmaBuffer)
            .map_err(DmaError::from_api)
    }

    /// 分配固定长度 typed coherent DMA array。
    pub fn array_zeroed<T: DmaValue>(
        &self,
        len: usize,
        align: usize,
        direction: DmaDirection,
    ) -> DmaResult<DmaArray<T>> {
        self.inner
            .array_zero_with_align::<T>(len, align, direction.into())
            .map(DmaArray)
            .map_err(DmaError::from_api)
    }

    /// 映射已有 slice 为 streaming DMA 区域。
    ///
    /// # Safety
    ///
    /// `buffer` 必须在返回的 mapping 生命周期内保持有效；调用方必须按目标 DMA
    /// 方向维护别名和同步规则，避免 CPU 与设备并发访问同一内存时破坏一致性。
    pub unsafe fn map_slice<T: DmaValue>(
        &self,
        buffer: &[T],
        align: usize,
        direction: DmaDirection,
    ) -> DmaResult<StreamingMapping<T>> {
        self.inner
            .map_single_array::<T>(buffer, align, direction.into())
            .map(StreamingMapping)
            .map_err(DmaError::from_api)
    }
}

/// 单个 typed coherent DMA buffer。
pub struct DmaBuffer<T: DmaValue>(dma_api::DBox<T>);

impl<T: DmaValue> DmaBuffer<T> {
    /// 返回设备可见 DMA 地址。
    pub fn dma_addr(&self) -> u64 {
        self.0.dma_addr().as_u64()
    }

    /// 返回 CPU 可访问指针。
    ///
    /// # Safety
    ///
    /// 返回的指针仅在 `self` 存活期间有效；解引用会绕过 wrapper 的 cache sync
    /// 语义，调用方必须确保不会违反设备/CPU 别名或同步规则。
    pub unsafe fn as_ptr(&self) -> NonNull<T> {
        self.0.as_ptr()
    }

    /// 读取值。
    pub fn read(&self) -> T {
        self.0.read()
    }

    /// 写入值。
    pub fn write(&mut self, value: T) {
        self.0.write(value);
    }

    /// 读改写值。
    pub fn modify(&mut self, f: impl FnOnce(&mut T)) {
        self.0.modify(f);
    }
}

/// typed coherent DMA array。
pub struct DmaArray<T: DmaValue>(dma_api::DArray<T>);

impl<T: DmaValue> DmaArray<T> {
    /// 返回设备可见 DMA 地址。
    pub fn dma_addr(&self) -> u64 {
        self.0.dma_addr().as_u64()
    }

    /// 返回元素数量。
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 判断数组是否为空。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 读取一个元素。
    pub fn read(&self, index: usize) -> Option<T> {
        self.0.read(index)
    }

    /// 写入一个元素。
    pub fn set(&mut self, index: usize, value: T) {
        let len = self.len();
        if index >= len {
            panic!("DMA 数组写入越界: index={index}, len={len}");
        }
        self.0.set(index, value);
    }

    /// 从 slice 复制数据到 DMA 数组。
    pub fn copy_from_slice(&mut self, source: &[T]) {
        self.0.copy_from_slice(source);
    }

    /// 在 CPU 读取前同步整个数组。
    pub fn prepare_read_all(&self) {
        self.0.prepare_read_all();
    }

    /// 在设备读取前同步整个数组。
    pub fn confirm_write_all(&self) {
        self.0.confirm_write_all();
    }
}

/// streaming DMA mapping。
pub struct StreamingMapping<T: DmaValue>(dma_api::SArrayPtr<T>);

impl<T: DmaValue> StreamingMapping<T> {
    /// 返回设备可见 DMA 地址。
    pub fn dma_addr(&self) -> u64 {
        self.0.dma_addr().as_u64()
    }

    /// 返回元素数量。
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 判断映射是否为空。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 读取一个元素，并按方向执行读取前同步。
    pub fn read(&self, index: usize) -> Option<T> {
        self.0.read(index)
    }

    /// 写入一个元素，并按方向执行写入后同步。
    pub fn set(&mut self, index: usize, value: T) {
        let len = self.len();
        if index >= len {
            panic!("DMA streaming 映射写入越界: index={index}, len={len}");
        }
        self.0.set(index, value);
    }

    /// 手动确认 CPU 写入对设备可见。
    pub fn copy_from_slice(&mut self, source: &[T]) {
        self.0.copy_from_slice(source);
    }

    /// 手动准备 CPU 读取设备写入。
    pub fn prepare_read_all(&self) {
        self.0.prepare_read_all();
    }

    /// 手动确认整个映射对设备可见。
    pub fn confirm_write_all(&self) {
        self.0.confirm_write_all();
    }
}
```

- [ ] **Step 2: Export typed wrappers**

Replace `crates/dma/src/lib.rs` with:

```rust
//! DMA 抽象层。
//!
//! 本 crate 是 SimpleKernel 对 `dma-api` 的唯一直接封装点。上层模块使用
//! `DmaDevice` / `DmaBuffer` / `DmaArray` / `StreamingMapping`，不要直接依赖
//! `dma_api::*` 类型。
//!
//! 当前后端是 QEMU VirtIO identity mapping 实现，不承诺 non-coherent 真机 DMA
//! cache/PTE 语义正确性。

#![no_std]

pub mod device;
pub mod direction;
pub mod error;
mod qemu;

pub use device::{DmaArray, DmaBuffer, DmaDevice, DmaValue, StreamingMapping};
pub use direction::DmaDirection;
pub use error::{DmaError, DmaResult};
pub use qemu::{raw_alloc_pages, raw_dealloc_pages, raw_map_single, raw_unmap_single};
```

- [ ] **Step 3: Check the crate**

Run:

```bash
devcontainer exec --workspace-folder . cargo check -p dma
```

Expected: the `dma` crate compiles and the command exits 0.

- [ ] **Step 4: Commit**

```bash
git add crates/dma/src/lib.rs crates/dma/src/device.rs
git commit --signoff -m "feat(dma): wrap dma-api typed buffers"
```

---

### Task 5: Route VirtIO HAL Through `crates/dma`

**Files:**
- Modify: `src/device/hal.rs`

- [ ] **Step 1: Replace imports**

In `src/device/hal.rs`, replace the import block:

```rust
use alloc::collections::BTreeMap;

use core::ptr::NonNull;

use memory_types::{PhysAddr, VirtAddr};
use sync::SpinLock;
use virtio_drivers::{BufferDirection, Hal};

use frame_allocator::AllocatedFrames;
```

with:

```rust
use core::ptr::NonNull;

use memory_types::PhysAddr;
use virtio_drivers::{BufferDirection, Hal};
```

- [ ] **Step 2: Remove local `DMA_TRACKER`**

Delete this static block from `src/device/hal.rs`:

```rust
/// DMA 分配追踪表——存储尚未释放的 DMA 帧，防止 Drop 自动回收。
///
/// key 为 DMA 缓冲区的物理地址（页对齐），value 为持有帧所有权的 `AllocatedFrames`。
static DMA_TRACKER: SpinLock<BTreeMap<u64, AllocatedFrames>> =
    SpinLock::new(BTreeMap::new(), "dma_tracker", sync::lock_level::DMA);
```

- [ ] **Step 3: Add direction conversion helper**

Add this helper above `unsafe impl Hal for SimpleKernelHal`:

```rust
fn virtio_direction(direction: BufferDirection) -> dma::DmaDirection {
    match direction {
        BufferDirection::DriverToDevice => dma::DmaDirection::ToDevice,
        BufferDirection::DeviceToDriver => dma::DmaDirection::FromDevice,
        BufferDirection::Both => dma::DmaDirection::Bidirectional,
    }
}
```

- [ ] **Step 4: Replace `dma_alloc` implementation**

Replace `dma_alloc` with:

```rust
fn dma_alloc(pages: usize, direction: BufferDirection) -> (u64, NonNull<u8>) {
    let direction = virtio_direction(direction);
    dma::raw_alloc_pages(pages, direction).unwrap_or_else(|error| {
        panic!("DMA 帧分配失败: pages={pages}, direction={direction:?}, error={error}");
    })
}
```

- [ ] **Step 5: Replace `dma_dealloc` implementation**

Replace `dma_dealloc` with:

```rust
unsafe fn dma_dealloc(paddr: u64, vaddr: NonNull<u8>, pages: usize) -> i32 {
    match dma::raw_dealloc_pages(paddr, vaddr, pages) {
        Ok(()) => 0,
        Err(error) => {
            log::error!("dma_dealloc: paddr={paddr:#x}, pages={pages}, error={error}");
            -1
        }
    }
}
```

- [ ] **Step 6: Replace `share` and `unshare`**

Replace `share` with:

```rust
unsafe fn share(buffer: NonNull<[u8]>, direction: BufferDirection) -> u64 {
    let direction = virtio_direction(direction);
    unsafe { dma::raw_map_single(buffer, direction) }.unwrap_or_else(|error| {
        panic!("DMA buffer 共享失败: direction={direction:?}, error={error}");
    })
}
```

Replace `unshare` with:

```rust
unsafe fn unshare(paddr: u64, buffer: NonNull<[u8]>, direction: BufferDirection) {
    dma::raw_unmap_single(paddr, buffer, virtio_direction(direction));
}
```

- [ ] **Step 7: Check root crate**

Run:

```bash
devcontainer exec --workspace-folder . cargo xtask check --arch riscv64
```

Expected: the command exits 0.

- [ ] **Step 8: Commit**

```bash
git add src/device/hal.rs
git commit --signoff \
  -m "refactor(device): route virtio dma through dma crate" \
  -m "Delegate VirtIO HAL allocation, deallocation, and streaming buffer mapping to the new dma crate raw helpers while preserving the current QEMU identity-mapped behavior."
```

---

### Task 6: Add ADR And Audit Progress Update

**Files:**
- Create: `docs/adr/014-qemu-virtio-dma-api-wrapper.md`
- Modify: `docs/adr/README.md`
- Modify: `docs/audit/audit-progress.md`
- Modify: `crates/dma/README.md`

- [ ] **Step 1: Create ADR-014**

Create `docs/adr/014-qemu-virtio-dma-api-wrapper.md`:

```markdown
# ADR-014: QEMU VirtIO DMA 抽象封装 `dma-api`

## 状态

提议

## 日期

2026-05-06

## 背景

当前 `virtio-drivers::Hal` 需要 DMA 分配、释放和 streaming buffer map/unmap。
旧实现直接在 `src/device/hal.rs` 中操作 `AllocatedFrames` 和 identity mapping，
导致 VirtIO 适配层同时承担 DMA 策略、内存分配和裸指针转换职责。

`dma-api` 已提供 `DeviceDma`、`DBox`、`DArray`、`SArrayPtr` 和 `DmaOp`，
可以复用 typed DMA 容器和 cache sync 调用形状。

## 决策

新增 `crates/dma`，作为 SimpleKernel 唯一直接依赖 `dma-api` 的封装层。
`crates/dma` 对外暴露 SimpleKernel 命名的 `DmaDevice`、`DmaBuffer<T>`、
`DmaArray<T>` 和 `StreamingMapping<T>`，并实现 QEMU VirtIO identity mapping
后端 `QemuIdentityDmaOp`。

`src/device/hal.rs` 只保留 `virtio-drivers::Hal` 适配逻辑，不直接依赖
`dma-api`、`AllocatedFrames` 或 DMA tracker。

## 后果

- 当前 QEMU VirtIO 行为保持不变。
- DMA 抽象边界从设备层移入 `crates/dma`。
- `dma-api` 类型不会扩散到 SimpleKernel 上层模块。
- 当前实现仍不承诺 non-coherent AArch64 真机 DMA 正确性。
- 后续真机支持需要继续设计 cache maintenance、PTE 属性和设备 DMA capability。
```

- [ ] **Step 2: Update ADR index**

Add this row to `docs/adr/README.md` after ADR-013:

```markdown
| 014 | [QEMU VirtIO DMA 抽象封装 `dma-api`](014-qemu-virtio-dma-api-wrapper.md) | 提议 | 2026-05-06 | R3/R6 |
```

- [ ] **Step 3: Update audit progress current status**

In `docs/audit/audit-progress.md`, update the current status section so the next target says:

```markdown
1. **DMA / VirtIO HAL 后续验证**：`crates/dma` 已封装 `dma-api` 并接入 QEMU VirtIO 路径；当前仍只承诺 QEMU VirtIO identity mapping，不承诺 non-coherent 真机 DMA。后续真机前需继续设计 cache maintenance、PTE 属性和设备 DMA capability。
```

Add this validation note after the DMA status paragraph:

```markdown
验证计划：执行 `cargo check -p dma`、`cargo xtask check --arch riscv64`、
`cargo xtask check --arch aarch64` 和 `cargo xtask test --arch riscv64 --name device-test`。
QEMU 测试需使用 30 秒超时并在超时后清理残留 `qemu-system` 进程。
```

- [ ] **Step 4: Expand `crates/dma/README.md`**

Append:

```markdown
## 第三方依赖边界

本 crate 内部使用 `dma-api 0.7.2`：

- `DmaDevice` 包装 `dma_api::DeviceDma`
- `DmaBuffer<T>` 包装 `dma_api::DBox<T>`
- `DmaArray<T>` 包装 `dma_api::DArray<T>`
- `StreamingMapping<T>` 包装 `dma_api::SArrayPtr<T>`

上层模块不要直接使用 `dma_api::*`。如果未来替换 DMA abstraction crate，只修改
`crates/dma`。
```

- [ ] **Step 5: Commit**

```bash
git add docs/adr/014-qemu-virtio-dma-api-wrapper.md docs/adr/README.md docs/audit/audit-progress.md crates/dma/README.md
git commit --signoff -m "docs(dma): record dma-api wrapper decision"
```

---

### Task 7: Full Validation

**Files:**
- Modify validation notes only when a command exposes a fact that should be recorded in `docs/audit/audit-progress.md`.

- [ ] **Step 1: Format check**

Run:

```bash
devcontainer exec --workspace-folder . cargo fmt --all -- --check
```

Expected: the command exits 0.

- [ ] **Step 2: RISC-V check**

Run:

```bash
devcontainer exec --workspace-folder . cargo xtask check --arch riscv64
```

Expected: the command exits 0.

- [ ] **Step 3: AArch64 check**

Run:

```bash
devcontainer exec --workspace-folder . cargo xtask check --arch aarch64
```

Expected: the command exits 0.

- [ ] **Step 4: VirtIO QEMU test**

Run with a 30 second shell timeout:

```bash
timeout 30s devcontainer exec --workspace-folder . cargo xtask test --arch riscv64 --name device-test
```

Expected: the command exits 0 and `device-test` reaches the VirtIO block check path.

- [ ] **Step 5: Clean up QEMU processes after timeout**

Run this cleanup command after a timeout:

```bash
pkill -f qemu-system
```

Expected: no residual QEMU process remains.

- [ ] **Step 6: Record validation evidence**

After Step 1 through Step 4 finish, update `docs/audit/audit-progress.md` with the exact command list and pass/fail result. Use this wording when every command passes:

```markdown
验证结果（2026-05-06）：`cargo check -p dma`、`cargo fmt --all -- --check`、
`cargo xtask check --arch riscv64`、`cargo xtask check --arch aarch64`、
`cargo xtask test --arch riscv64 --name device-test` 均通过。
```

- [ ] **Step 7: Commit validation note**

```bash
git add docs/audit/audit-progress.md
git commit --signoff -m "docs(audit): update dma validation status"
```
