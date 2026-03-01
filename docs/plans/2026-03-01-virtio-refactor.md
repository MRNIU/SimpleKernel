# VirtIO Refactor: Directory Restructure + std:: Modernization

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reorganize `src/device/virtio/device/` into per-type subdirectories (`blk/`, `console/`, `net/`, `gpu/`, `input/`) and replace raw C arrays / manual bitmasks in `virtio_blk.hpp` with `std::array`, `std::bitset`, and `std::span`.

**Architecture:** Two-phase mechanical + semantic refactor. Phase 1 moves files and updates all include paths—purely mechanical, zero logic change. Phase 2 modernizes `VirtioBlk<>` internals using `std::array`/`std::bitset` while keeping identical runtime behavior.

**Tech Stack:** C++23 freestanding, CMake, `std::array`/`std::bitset`/`std::span` (available in C++23 freestanding), clang-format, pre-commit.

---

## Files Involved

| File | Role |
|------|------|
| `src/device/virtio/device/virtio_blk.hpp` | Main VirtioBlk template (840 lines) — primary target for std:: modernization |
| `src/device/virtio/device/virtio_blk_defs.h` | Block-specific protocol constants |
| `src/device/virtio/device/virtio_blk_vfs_adapter.hpp` | VFS adapter wrapper |
| `src/device/virtio/device/device_initializer.hpp` | Cross-device initializer — stays in `device/` root |
| `src/device/virtio/device/virtio_console.h` | Placeholder |
| `src/device/virtio/device/virtio_gpu.h` | Placeholder |
| `src/device/virtio/device/virtio_net.h` | Placeholder |
| `src/device/virtio/device/virtio_input.h` | Placeholder |
| `src/device/virtio/virtio_driver.hpp` | Includes `virtio/device/virtio_blk.hpp` and `virtio/device/virtio_blk_vfs_adapter.hpp` — needs path updates |
| `src/device/virtio/CMakeLists.txt` | Build definition — no changes needed (include root is `device/`) |

---

## Phase 1: Directory Restructure

### Task 1: Create subdirectory skeleton

**Files:**
- Create dirs: `src/device/virtio/device/blk/`, `src/device/virtio/device/console/`, `src/device/virtio/device/net/`, `src/device/virtio/device/gpu/`, `src/device/virtio/device/input/`

**Step 1: Create directories**

```bash
mkdir -p src/device/virtio/device/blk
mkdir -p src/device/virtio/device/console
mkdir -p src/device/virtio/device/net
mkdir -p src/device/virtio/device/gpu
mkdir -p src/device/virtio/device/input
```

Expected: all 5 directories exist, no errors.

**Step 2: Verify**

```bash
ls src/device/virtio/device/
```

Expected: `blk/  console/  device_initializer.hpp  gpu/  input/  net/  virtio_blk.hpp  virtio_blk_defs.h  virtio_blk_vfs_adapter.hpp  ...`

---

### Task 2: Move blk files

**Files:**
- Move: `src/device/virtio/device/virtio_blk.hpp` → `src/device/virtio/device/blk/virtio_blk.hpp`
- Move: `src/device/virtio/device/virtio_blk_defs.h` → `src/device/virtio/device/blk/virtio_blk_defs.h`
- Move: `src/device/virtio/device/virtio_blk_vfs_adapter.hpp` → `src/device/virtio/device/blk/virtio_blk_vfs_adapter.hpp`

**Step 1: Move files**

```bash
git mv src/device/virtio/device/virtio_blk.hpp        src/device/virtio/device/blk/virtio_blk.hpp
git mv src/device/virtio/device/virtio_blk_defs.h     src/device/virtio/device/blk/virtio_blk_defs.h
git mv src/device/virtio/device/virtio_blk_vfs_adapter.hpp  src/device/virtio/device/blk/virtio_blk_vfs_adapter.hpp
```

**Step 2: Verify git tracks the move**

```bash
git status
```

Expected: shows `renamed: src/device/virtio/device/virtio_blk.hpp -> src/device/virtio/device/blk/virtio_blk.hpp` etc.

---

### Task 3: Move placeholder files

**Files:**
- Move: `src/device/virtio/device/virtio_console.h` → `src/device/virtio/device/console/virtio_console.h`
- Move: `src/device/virtio/device/virtio_net.h` → `src/device/virtio/device/net/virtio_net.h`
- Move: `src/device/virtio/device/virtio_gpu.h` → `src/device/virtio/device/gpu/virtio_gpu.h`
- Move: `src/device/virtio/device/virtio_input.h` → `src/device/virtio/device/input/virtio_input.h`

**Step 1: Move placeholder files**

```bash
git mv src/device/virtio/device/virtio_console.h  src/device/virtio/device/console/virtio_console.h
git mv src/device/virtio/device/virtio_net.h      src/device/virtio/device/net/virtio_net.h
git mv src/device/virtio/device/virtio_gpu.h      src/device/virtio/device/gpu/virtio_gpu.h
git mv src/device/virtio/device/virtio_input.h    src/device/virtio/device/input/virtio_input.h
```

**Step 2: Verify**

```bash
ls src/device/virtio/device/console/
ls src/device/virtio/device/net/
ls src/device/virtio/device/gpu/
ls src/device/virtio/device/input/
```

Expected: each dir contains exactly one `.h` file.

---

### Task 4: Update include paths in blk/ files

The moved files include each other. Must update their internal `#include` directives.

**Files:**
- Modify: `src/device/virtio/device/blk/virtio_blk.hpp` — line 17: `"virtio/device/device_initializer.hpp"` → no change needed (relative to device/ root include)
- Modify: `src/device/virtio/device/blk/virtio_blk.hpp` — line 18: `"virtio/device/virtio_blk_defs.h"` → `"virtio/device/blk/virtio_blk_defs.h"`
- Modify: `src/device/virtio/device/blk/virtio_blk_vfs_adapter.hpp` — line 10: `"virtio/device/virtio_blk.hpp"` → `"virtio/device/blk/virtio_blk.hpp"`

**Step 1: Update virtio_blk.hpp include**

In `src/device/virtio/device/blk/virtio_blk.hpp`, change:
```cpp
// before:
#include "virtio/device/virtio_blk_defs.h"
// after:
#include "virtio/device/blk/virtio_blk_defs.h"
```

**Step 2: Update virtio_blk_vfs_adapter.hpp include**

In `src/device/virtio/device/blk/virtio_blk_vfs_adapter.hpp`, change:
```cpp
// before:
#include "virtio/device/virtio_blk.hpp"
// after:
#include "virtio/device/blk/virtio_blk.hpp"
```

**Step 3: Update header guards**

Update `#ifndef` / `#define` guards to reflect new paths:

- `virtio_blk.hpp`: `SIMPLEKERNEL_SRC_DEVICE_VIRTIO_DEVICE_VIRTIO_BLK_HPP_` → `SIMPLEKERNEL_SRC_DEVICE_VIRTIO_DEVICE_BLK_VIRTIO_BLK_HPP_`
- `virtio_blk_defs.h`: `SIMPLEKERNEL_SRC_DEVICE_VIRTIO_DEVICE_VIRTIO_BLK_DEFS_H_` → `SIMPLEKERNEL_SRC_DEVICE_VIRTIO_DEVICE_BLK_VIRTIO_BLK_DEFS_H_`
- `virtio_blk_vfs_adapter.hpp`: `SIMPLEKERNEL_SRC_DEVICE_VIRTIO_DEVICE_VIRTIO_BLK_VFS_ADAPTER_HPP_` → `SIMPLEKERNEL_SRC_DEVICE_VIRTIO_DEVICE_BLK_VIRTIO_BLK_VFS_ADAPTER_HPP_`

---

### Task 5: Update include paths in virtio_driver.hpp

**Files:**
- Modify: `src/device/virtio/virtio_driver.hpp` — update two include paths

**Step 1: Update includes in virtio_driver.hpp**

```cpp
// before:
#include "virtio/device/virtio_blk.hpp"
#include "virtio/device/virtio_blk_vfs_adapter.hpp"
// after:
#include "virtio/device/blk/virtio_blk.hpp"
#include "virtio/device/blk/virtio_blk_vfs_adapter.hpp"
```

---

### Task 6: Build verification after Phase 1

**Step 1: Configure and build**

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel 2>&1 | tail -30
```

Expected: zero errors. If include errors appear, the path update in Task 4-5 missed a file.

**Step 2: Check for any remaining stale includes**

```bash
grep -r "virtio/device/virtio_blk" src/ --include="*.hpp" --include="*.h" --include="*.cpp"
```

Expected: empty output (all updated to `virtio/device/blk/virtio_blk`).

**Step 3: Commit Phase 1**

```bash
git add -A
git commit --signoff -m "refactor(virtio): reorganize device/ into per-type subdirectories

Move virtio_blk.hpp, virtio_blk_defs.h, virtio_blk_vfs_adapter.hpp into
device/blk/, and placeholder files into console/, net/, gpu/, input/.
Update all include paths and header guards accordingly."
```

---

## Phase 2: std:: Modernization in VirtioBlk

### Context

In `src/device/virtio/device/blk/virtio_blk.hpp`, three raw constructs will be replaced:

| Before | After | Rationale |
|--------|-------|-----------|
| `RequestSlot slots_[kMaxInflight]` | `std::array<RequestSlot, kMaxInflight> slots_` | Bounds-safe, no decay |
| `uint64_t slot_bitmap_` | `std::bitset<kMaxInflight> slot_bitmap_` | Semantic clarity, named ops |
| `IoVec readable_iovs[kMaxSgElements]` / `writable_iovs` (local in methods) | `std::array<IoVec, kMaxSgElements>` | Consistent style |
| `static const char* const kNames[]` | `static constexpr std::array<const char*, 4> kNames` | type-safe, constexpr |

**Note on `std::bitset`:** `AllocRequestSlot()` uses `__builtin_ctzll` on the raw `uint64_t` for O(1) slot finding. `std::bitset` does not expose a `find_first_zero` directly. We preserve the bitwise trick by extracting the underlying bits via `slot_bitmap_.to_ulong()`.

---

### Task 7: Add `<array>` and `<bitset>` includes

**Files:**
- Modify: `src/device/virtio/device/blk/virtio_blk.hpp`

**Step 1: Add includes**

After the existing `#include <cstdint>` block (lines 10-12), add:
```cpp
#include <array>
#include <bitset>
```

---

### Task 8: Replace `slots_` member with `std::array`

**Files:**
- Modify: `src/device/virtio/device/blk/virtio_blk.hpp`

**Step 1: Change member declaration**

Find (line ~829):
```cpp
  RequestSlot slots_[kMaxInflight];
```
Replace with:
```cpp
  std::array<RequestSlot, kMaxInflight> slots_{};
```

**Step 2: Update `CopySlots()`**

`CopySlots()` iterates `slots_` with `slots_[i]` — `std::array` supports `operator[]`, so no change needed in the loop body. Verify no raw array semantics are used elsewhere by searching:

```bash
grep -n "slots_\[" src/device/virtio/device/blk/virtio_blk.hpp
```

All uses of `slots_[idx]` remain valid with `std::array`.

---

### Task 9: Replace `slot_bitmap_` with `std::bitset<kMaxInflight>`

**Files:**
- Modify: `src/device/virtio/device/blk/virtio_blk.hpp`

**Step 1: Change member declaration**

Find (line ~831):
```cpp
  uint64_t slot_bitmap_;
```
Replace with:
```cpp
  std::bitset<kMaxInflight> slot_bitmap_{};
```

**Step 2: Update `AllocRequestSlot()`**

Current code (lines ~655-666):
```cpp
[[nodiscard]] auto AllocRequestSlot() -> Expected<uint16_t> {
  uint64_t free_bits = ~slot_bitmap_;
  if (free_bits == 0) {
    return std::unexpected(Error{ErrorCode::kNoFreeDescriptors});
  }
  auto idx = static_cast<uint16_t>(__builtin_ctzll(free_bits));
  if (idx >= kMaxInflight) {
    return std::unexpected(Error{ErrorCode::kNoFreeDescriptors});
  }
  slot_bitmap_ |= (uint64_t{1} << idx);
  return idx;
}
```

Replace with (preserves identical behavior, uses `std::bitset` API):
```cpp
[[nodiscard]] auto AllocRequestSlot() -> Expected<uint16_t> {
  if (slot_bitmap_.all()) {
    return std::unexpected(Error{ErrorCode::kNoFreeDescriptors});
  }
  // Use __builtin_ctzll on the complement for O(1) first-zero detection
  uint64_t free_bits = ~slot_bitmap_.to_ulong();
  auto idx = static_cast<uint16_t>(__builtin_ctzll(free_bits));
  slot_bitmap_.set(idx);
  return idx;
}
```

**Step 3: Update `FreeRequestSlot()`**

Current code (lines ~673-677):
```cpp
auto FreeRequestSlot(uint16_t idx) -> void {
  if (idx < kMaxInflight) {
    slot_bitmap_ &= ~(uint64_t{1} << idx);
  }
}
```

Replace with:
```cpp
auto FreeRequestSlot(uint16_t idx) -> void {
  if (idx < kMaxInflight) {
    slot_bitmap_.reset(idx);
  }
}
```

**Step 4: Update `FindSlotByDescHead()`**

Current code (lines ~685-695):
```cpp
[[nodiscard]] auto FindSlotByDescHead(uint16_t desc_head) const -> uint16_t {
  uint64_t used = slot_bitmap_;
  while (used != 0) {
    auto i = static_cast<uint16_t>(__builtin_ctzll(used));
    if (slots_[i].desc_head == desc_head) {
      return i;
    }
    used &= used - 1;  // clear lowest set bit
  }
  return kMaxInflight;
}
```

Replace with (same algorithm, explicitly converts to underlying bits):
```cpp
[[nodiscard]] auto FindSlotByDescHead(uint16_t desc_head) const -> uint16_t {
  uint64_t used = slot_bitmap_.to_ulong();
  while (used != 0) {
    auto i = static_cast<uint16_t>(__builtin_ctzll(used));
    if (slots_[i].desc_head == desc_head) {
      return i;
    }
    used &= used - 1;  // clear lowest set bit
  }
  return kMaxInflight;
}
```

---

### Task 10: Replace local IoVec arrays in Read/Write with `std::array`

**Files:**
- Modify: `src/device/virtio/device/blk/virtio_blk.hpp`

**Step 1: Find the local array declarations**

```bash
grep -n "IoVec.*\[" src/device/virtio/device/blk/virtio_blk.hpp
```

Look for patterns like:
```cpp
IoVec readable_iovs[kMaxSgElements];
IoVec writable_iovs[kMaxSgElements];
```

**Step 2: Replace each local declaration**

Change each occurrence of:
```cpp
IoVec readable_iovs[kMaxSgElements];
```
to:
```cpp
std::array<IoVec, kMaxSgElements> readable_iovs{};
```

And similarly for `writable_iovs`. Array element access `readable_iovs[i]` remains valid.

If the arrays are passed to a function taking `const IoVec*`, add `.data()`:
```cpp
// before (if present):
SomeFunc(readable_iovs, count);
// after:
SomeFunc(readable_iovs.data(), count);
```

---

### Task 11: Replace `kNames[]` in `VirtioBlkVfsAdapter::GetName()`

**Files:**
- Modify: `src/device/virtio/device/blk/virtio_blk_vfs_adapter.hpp`

**Step 1: Add include**

Add at top (with other `<...>` includes):
```cpp
#include <array>
```

**Step 2: Replace static array**

Find (lines ~60-62):
```cpp
static const char* const kNames[] = {"virtio-blk0", "virtio-blk1",
                                     "virtio-blk2", "virtio-blk3"};
return (index_ < 4) ? kNames[index_] : "virtio-blk?";
```

Replace with:
```cpp
static constexpr std::array<const char*, 4> kNames = {
    "virtio-blk0", "virtio-blk1", "virtio-blk2", "virtio-blk3"};
return (index_ < kNames.size()) ? kNames[index_] : "virtio-blk?";
```

---

### Task 12: Build verification after Phase 2

**Step 1: Build**

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel 2>&1 | tail -40
```

Expected: zero errors, zero warnings introduced by refactor.

**Step 2: Run unit tests**

```bash
cd build_riscv64 && make unit-test 2>&1 | tail -20
```

Expected: all tests pass (or report pre-existing failures, not new ones).

**Step 3: Run pre-commit (format check)**

```bash
pre-commit run --all-files
```

Expected: passes. If clang-format makes auto-fixes, `git add` them.

**Step 4: Commit Phase 2**

```bash
git add -A
git commit --signoff -m "refactor(virtio): modernize VirtioBlk with std::array and std::bitset

Replace raw C arrays slots_[kMaxInflight] with std::array, uint64_t
slot_bitmap_ with std::bitset<64>, and local IoVec[] with std::array.
Replace static kNames[] with constexpr std::array. No behavior change."
```

---

## Verification Checklist

After both phases are complete:

- [ ] `src/device/virtio/device/blk/` contains: `virtio_blk.hpp`, `virtio_blk_defs.h`, `virtio_blk_vfs_adapter.hpp`
- [ ] `src/device/virtio/device/{console,net,gpu,input}/` each contain their placeholder header
- [ ] `src/device/virtio/device/device_initializer.hpp` remains at `device/` root
- [ ] No `#include "virtio/device/virtio_blk"` (old path) in any source file
- [ ] Header guards updated to reflect new paths
- [ ] `VirtioBlk::slots_` is `std::array<RequestSlot, kMaxInflight>`
- [ ] `VirtioBlk::slot_bitmap_` is `std::bitset<kMaxInflight>`
- [ ] `AllocRequestSlot()` uses `slot_bitmap_.all()` and `slot_bitmap_.set()`
- [ ] `FreeRequestSlot()` uses `slot_bitmap_.reset()`
- [ ] `FindSlotByDescHead()` uses `slot_bitmap_.to_ulong()`
- [ ] Local `IoVec` arrays are `std::array<IoVec, kMaxSgElements>`
- [ ] `GetName()` uses `constexpr std::array<const char*, 4>`
- [ ] Build: zero errors
- [ ] Tests: no regressions
- [ ] pre-commit: clean
