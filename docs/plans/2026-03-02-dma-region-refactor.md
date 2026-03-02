# DMA Region Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace raw `void*` + identity-mapping `reinterpret_cast` DMA handling in VirtIO subsystem with a type-safe `DmaRegion` value type, dedicated slot DMA allocation, and a `VirtToPhys` abstraction.

**Architecture:** Introduce `DmaRegion{virt, phys, size}` as the universal DMA memory descriptor. Push all address translation behind a `VirtToPhysFunc` callback defaulting to identity mapping. Isolate the identity-mapping assumption to a single call site (`VirtioDriver::Probe`). Allocate `RequestSlot` arrays from explicit DMA memory instead of relying on object memory being DMA-accessible.

**Tech Stack:** C++23, freestanding, header-only (except `io_buffer.cpp` and `virtio_driver.cpp`), GoogleTest for unit tests, no heap allocation in headers.

---

## File Reference

| File | Role |
|------|------|
| `src/include/dma_region.hpp` | **NEW** — DmaRegion + VirtToPhysFunc |
| `src/device/virtio/virt_queue/split.hpp` | Modify constructor to accept DmaRegion |
| `src/device/virtio/device/blk/virtio_blk.hpp` | Modify Create(), Read(), Write(), DoEnqueue() |
| `src/device/virtio/virtio_driver.hpp` | Add slot_buffers_ array |
| `src/device/virtio/virtio_driver.cpp` | Construct DmaRegion, pass to Create() |
| `src/include/io_buffer.hpp` | Add ToDmaRegion() convenience method |
| `src/io_buffer.cpp` | Implement ToDmaRegion() |
| `tests/unit_test/dma_region_test.cpp` | **NEW** — DmaRegion unit tests |
| `tests/unit_test/mocks/io_buffer_mock.cpp` | Add ToDmaRegion() mock |
| `tests/unit_test/CMakeLists.txt` | Add dma_region_test.cpp |

## Conventions Reminder

- Google C++ style, `.clang-format` enforced
- Header guards: `#ifndef SIMPLEKERNEL_SRC_INCLUDE_FILENAME_HPP_`
- Copyright: `/** @copyright Copyright The SimpleKernel Contributors */`
- Trailing return types: `auto Func() -> RetType`
- Members: `snake_case_` (trailing underscore)
- Constants: `kCamelCase`
- `Expected<T>` for errors, no exceptions/RTTI
- Doxygen `@brief`/`@pre`/`@post` on interface methods
- Includes: system → third-party → project

---

### Task 1: Create DmaRegion header

**Files:**
- Create: `src/include/dma_region.hpp`

**Step 1: Create the header file**

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_DMA_REGION_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_DMA_REGION_HPP_

#include <cstddef>
#include <cstdint>

#include "expected.hpp"

/// @brief Address translation callback type
/// @param virt Virtual address to translate
/// @return Corresponding physical address
using VirtToPhysFunc = auto (*)(uintptr_t virt) -> uintptr_t;

/// @brief Identity mapping: phys == virt (default for early boot / no MMU)
[[nodiscard]] inline auto IdentityVirtToPhys(uintptr_t virt) -> uintptr_t {
  return virt;
}

/**
 * @brief Non-owning descriptor for a DMA-accessible memory region
 *
 * Bundles virtual address, physical (bus) address, and size into a single
 * value type. Does NOT own the memory — lifetime is managed by the allocator
 * (e.g. IoBuffer).
 *
 * @note POD-like, trivially copyable, safe to pass by value or const-ref.
 */
struct DmaRegion {
  /// Virtual (CPU-accessible) base address
  void* virt{nullptr};
  /// Physical (bus/DMA) base address
  uintptr_t phys{0};
  /// Region size in bytes
  size_t size{0};

  /// @brief Check if the region is valid (non-null, non-zero size)
  [[nodiscard]] auto IsValid() const -> bool {
    return virt != nullptr && size > 0;
  }

  /// @brief Get typed pointer to the virtual base
  [[nodiscard]] auto data() const -> uint8_t* {
    return static_cast<uint8_t*>(virt);
  }

  /**
   * @brief Create a sub-region at the given offset
   *
   * @param offset Byte offset from the start of this region
   * @param len Byte length of the sub-region
   * @return Sub-region on success, error if out of bounds
   *
   * @pre offset + len <= size
   * @post Returned region shares the same underlying memory
   */
  [[nodiscard]] auto SubRegion(size_t offset, size_t len) const
      -> Expected<DmaRegion> {
    if (offset + len > size) {
      return std::unexpected(Error{ErrorCode::kInvalidArgument});
    }
    return DmaRegion{
        .virt = static_cast<uint8_t*>(virt) + offset,
        .phys = phys + offset,
        .size = len,
    };
  }
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_DMA_REGION_HPP_
```

**Step 2: Verify it compiles**

Run: `pre-commit run --all-files` (format check only — no .cpp needed, header-only)

**Step 3: Commit**

```bash
git add src/include/dma_region.hpp
git commit --signoff -m "feat(device): add DmaRegion value type and VirtToPhysFunc"
```

---

### Task 2: Write DmaRegion unit tests

**Files:**
- Create: `tests/unit_test/dma_region_test.cpp`
- Modify: `tests/unit_test/CMakeLists.txt`

**Step 1: Write the tests**

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#include "dma_region.hpp"

#include <gtest/gtest.h>

#include <cstring>

TEST(DmaRegionTest, DefaultConstructedIsInvalid) {
  DmaRegion region{};
  EXPECT_FALSE(region.IsValid());
  EXPECT_EQ(region.virt, nullptr);
  EXPECT_EQ(region.phys, 0u);
  EXPECT_EQ(region.size, 0u);
}

TEST(DmaRegionTest, ValidRegion) {
  uint8_t buf[256]{};
  DmaRegion region{
      .virt = buf,
      .phys = 0x8000'0000,
      .size = sizeof(buf),
  };
  EXPECT_TRUE(region.IsValid());
  EXPECT_EQ(region.data(), buf);
  EXPECT_EQ(region.phys, 0x8000'0000u);
  EXPECT_EQ(region.size, 256u);
}

TEST(DmaRegionTest, NullVirtIsInvalid) {
  DmaRegion region{.virt = nullptr, .phys = 0x1000, .size = 4096};
  EXPECT_FALSE(region.IsValid());
}

TEST(DmaRegionTest, ZeroSizeIsInvalid) {
  uint8_t buf[1]{};
  DmaRegion region{.virt = buf, .phys = 0x1000, .size = 0};
  EXPECT_FALSE(region.IsValid());
}

TEST(DmaRegionTest, SubRegionSuccess) {
  uint8_t buf[4096]{};
  DmaRegion region{
      .virt = buf,
      .phys = 0x1'0000,
      .size = 4096,
  };

  auto sub = region.SubRegion(256, 512);
  ASSERT_TRUE(sub.has_value());
  EXPECT_EQ(sub->virt, buf + 256);
  EXPECT_EQ(sub->phys, 0x1'0000u + 256);
  EXPECT_EQ(sub->size, 512u);
  EXPECT_TRUE(sub->IsValid());
}

TEST(DmaRegionTest, SubRegionOutOfBoundsFails) {
  uint8_t buf[256]{};
  DmaRegion region{.virt = buf, .phys = 0x1000, .size = 256};

  auto sub = region.SubRegion(200, 100);  // 200 + 100 > 256
  EXPECT_FALSE(sub.has_value());
  EXPECT_EQ(sub.error().code, ErrorCode::kInvalidArgument);
}

TEST(DmaRegionTest, SubRegionExactBoundarySucceeds) {
  uint8_t buf[256]{};
  DmaRegion region{.virt = buf, .phys = 0x1000, .size = 256};

  auto sub = region.SubRegion(0, 256);  // exact fit
  ASSERT_TRUE(sub.has_value());
  EXPECT_EQ(sub->size, 256u);
}

TEST(DmaRegionTest, IdentityVirtToPhys) {
  uintptr_t addr = 0xDEAD'BEEF;
  EXPECT_EQ(IdentityVirtToPhys(addr), addr);
}
```

**Step 2: Add to CMakeLists.txt**

Add `dma_region_test.cpp` to the `ADD_EXECUTABLE` source list in `tests/unit_test/CMakeLists.txt`, after `virtio_driver_test.cpp`.

**Step 3: Build and run tests**

Run:
```bash
cmake --preset build_x86_64
cd build_x86_64 && make unit-test
```

Expected: All DmaRegion tests PASS.

**Step 4: Commit**

```bash
git add tests/unit_test/dma_region_test.cpp tests/unit_test/CMakeLists.txt
git commit --signoff -m "test(device): add DmaRegion unit tests"
```

---

### Task 3: Add IoBuffer::ToDmaRegion()

**Files:**
- Modify: `src/include/io_buffer.hpp` (add method declaration)
- Modify: `src/io_buffer.cpp` (add implementation)
- Modify: `tests/unit_test/mocks/io_buffer_mock.cpp` (add mock implementation)

**Step 1: Add include and method declaration to io_buffer.hpp**

Add `#include "dma_region.hpp"` to the includes. Add after `IsValid()`:

```cpp
  /**
   * @brief Create a DmaRegion view of this buffer
   *
   * @param v2p Address translation function (default: identity mapping)
   * @return DmaRegion describing this buffer's memory
   *
   * @pre IsValid() == true
   * @post Returned DmaRegion does not own the memory
   */
  [[nodiscard]] auto ToDmaRegion(VirtToPhysFunc v2p = IdentityVirtToPhys) const
      -> DmaRegion;
```

**Step 2: Add implementation to io_buffer.cpp**

```cpp
auto IoBuffer::ToDmaRegion(VirtToPhysFunc v2p) const -> DmaRegion {
  return DmaRegion{
      .virt = data_,
      .phys = v2p(reinterpret_cast<uintptr_t>(data_)),
      .size = size_,
  };
}
```

**Step 3: Add mock implementation to io_buffer_mock.cpp**

```cpp
auto IoBuffer::ToDmaRegion(VirtToPhysFunc v2p) const -> DmaRegion {
  return DmaRegion{
      .virt = data_,
      .phys = v2p(reinterpret_cast<uintptr_t>(data_)),
      .size = size_,
  };
}
```

**Step 4: Build and run tests**

Run:
```bash
cd build_x86_64 && make unit-test
```

Expected: All existing tests still PASS. No regressions.

**Step 5: Commit**

```bash
git add src/include/io_buffer.hpp src/io_buffer.cpp tests/unit_test/mocks/io_buffer_mock.cpp
git commit --signoff -m "feat(device): add IoBuffer::ToDmaRegion() method"
```

---

### Task 4: Refactor SplitVirtqueue constructor to accept DmaRegion

**Files:**
- Modify: `src/device/virtio/virt_queue/split.hpp`

**Step 1: Add DmaRegion include**

Add `#include "dma_region.hpp"` to the includes (after `"expected.hpp"`).

**Step 2: Change constructor signature and body**

Replace the constructor signature (line 240-241):

```cpp
// FROM:
SplitVirtqueue(void* dma_buf, uint64_t phys_base, uint16_t queue_size,
               bool event_idx, size_t used_align = Used::kAlign)

// TO:
SplitVirtqueue(const DmaRegion& dma, uint16_t queue_size,
               bool event_idx, size_t used_align = Used::kAlign)
```

Replace constructor body initialization (lines 242-244):

```cpp
// FROM:
    : queue_size_(queue_size),
      phys_base_(phys_base),
      event_idx_enabled_(event_idx) {
  if (dma_buf == nullptr || !IsPowerOfTwo(queue_size)) {

// TO:
    : queue_size_(queue_size),
      phys_base_(dma.phys),
      event_idx_enabled_(event_idx) {
  if (!dma.IsValid() || !IsPowerOfTwo(queue_size)) {
```

Replace line 263 (base pointer):

```cpp
// FROM:
auto* base = static_cast<uint8_t*>(dma_buf);

// TO:
auto* base = dma.data();
```

**Step 3: Build to verify**

Run: `cd build_x86_64 && make unit-test` — this will fail because callers still pass `void*`. That's expected; we fix callers in Task 5.

**Step 4: Commit (WIP)**

Do NOT commit yet — proceed to Task 5 to fix callers atomically.

---

### Task 5: Refactor VirtioBlk::Create() to accept DmaRegion + slot DmaRegion + VirtToPhysFunc

**Files:**
- Modify: `src/device/virtio/device/blk/virtio_blk.hpp`

This is the largest single change. Modify in this order:

**Step 1: Add DmaRegion include**

Add `#include "dma_region.hpp"` to the includes.

**Step 2: Add GetRequiredSlotMemSize() static method**

Add after `CalcDmaSize()`:

```cpp
  /**
   * @brief 计算 RequestSlot DMA 内存所需字节数
   *
   * @return pair.first = 总字节数，pair.second = 对齐要求（字节）
   */
  [[nodiscard]] static constexpr auto GetRequiredSlotMemSize()
      -> std::pair<size_t, size_t> {
    return {sizeof(RequestSlot) * kMaxInflight, alignof(RequestSlot)};
  }
```

**Step 3: Change Create() signature**

```cpp
// FROM:
static auto Create(uint64_t mmio_base, void* vq_dma_buf,
                   uint16_t queue_count = 1,
                   uint32_t queue_size = 128,
                   uint64_t driver_features = 0) -> Expected<VirtioBlk>

// TO:
static auto Create(uint64_t mmio_base,
                   const DmaRegion& vq_dma,
                   const DmaRegion& slot_dma,
                   VirtToPhysFunc virt_to_phys = IdentityVirtToPhys,
                   uint16_t queue_count = 1,
                   uint32_t queue_size = 128,
                   uint64_t driver_features = 0) -> Expected<VirtioBlk>
```

**Step 4: Update Create() body — virtqueue construction (lines 150-156)**

```cpp
// FROM:
uint64_t dma_phys = reinterpret_cast<uintptr_t>(vq_dma_buf);
VirtqueueT vq(vq_dma_buf, dma_phys, static_cast<uint16_t>(queue_size),
              event_idx);

// TO:
VirtqueueT vq(vq_dma, static_cast<uint16_t>(queue_size), event_idx);
```

**Step 5: Update Create() return — pass new members**

```cpp
// FROM:
return VirtioBlk(std::move(transport), std::move(vq), negotiated);

// TO:
return VirtioBlk(std::move(transport), std::move(vq), negotiated,
                 slot_dma, virt_to_phys);
```

**Step 6: Update private constructor**

```cpp
// FROM:
VirtioBlk(TransportT transport, VirtqueueT vq, uint64_t features)
    : transport_(std::move(transport)),
      vq_(std::move(vq)),
      negotiated_features_(features),
      stats_{},
      slot_bitmap_(0),
      old_avail_idx_(0),
      request_completed_(false) {}

// TO:
VirtioBlk(TransportT transport, VirtqueueT vq, uint64_t features,
          const DmaRegion& slot_dma, VirtToPhysFunc v2p)
    : transport_(std::move(transport)),
      vq_(std::move(vq)),
      negotiated_features_(features),
      slot_dma_(slot_dma),
      slots_(reinterpret_cast<RequestSlot*>(slot_dma.data())),
      virt_to_phys_(v2p),
      stats_{},
      slot_bitmap_(0),
      old_avail_idx_(0),
      request_completed_(false) {}
```

**Step 7: Change slots_ from embedded array to pointer**

```cpp
// FROM:
std::array<RequestSlot, kMaxInflight> slots_{};

// TO:
/// DMA region backing the request slot pool
DmaRegion slot_dma_;
/// Pointer to request slot array (lives in slot_dma_ memory)
RequestSlot* slots_{nullptr};
/// Address translation callback
VirtToPhysFunc virt_to_phys_{IdentityVirtToPhys};
```

**Step 8: Fix DoEnqueue() — header and status IoVec (lines 575-591)**

```cpp
// FROM:
readable_iovs[readable_count++] = {
    reinterpret_cast<uintptr_t>(&slot.header), sizeof(BlkReqHeader)};
// ...
writable_iovs[writable_count++] = {
    reinterpret_cast<uintptr_t>(const_cast<uint8_t*>(&slot.status)),
    sizeof(uint8_t)};

// TO:
auto slot_base_phys =
    slot_dma_.phys + static_cast<size_t>(slot_idx) * sizeof(RequestSlot);
readable_iovs[readable_count++] = {
    slot_base_phys + offsetof(RequestSlot, header), sizeof(BlkReqHeader)};
// ...
writable_iovs[writable_count++] = {
    slot_base_phys + offsetof(RequestSlot, status), sizeof(uint8_t)};
```

**Step 9: Fix Read() and Write() — data buffer IoVec (lines 318, 338)**

```cpp
// FROM (Read):
IoVec data_iov{reinterpret_cast<uintptr_t>(data), kSectorSize};

// TO (Read):
IoVec data_iov{virt_to_phys_(reinterpret_cast<uintptr_t>(data)), kSectorSize};

// FROM (Write):
IoVec data_iov{reinterpret_cast<uintptr_t>(const_cast<uint8_t*>(data)),
               kSectorSize};

// TO (Write):
IoVec data_iov{virt_to_phys_(reinterpret_cast<uintptr_t>(const_cast<uint8_t*>(data))),
               kSectorSize};
```

**Step 10: Fix move constructor and move assignment**

Update to copy `slot_dma_`, `slots_`, `virt_to_phys_` and invalidate the source:

```cpp
VirtioBlk(VirtioBlk&& other) noexcept
    : transport_(std::move(other.transport_)),
      vq_(std::move(other.vq_)),
      negotiated_features_(other.negotiated_features_),
      slot_dma_(other.slot_dma_),
      slots_(other.slots_),
      virt_to_phys_(other.virt_to_phys_),
      stats_(other.stats_),
      slot_bitmap_(other.slot_bitmap_),
      old_avail_idx_(other.old_avail_idx_),
      request_completed_(other.request_completed_) {
  other.slots_ = nullptr;
  other.slot_bitmap_ = 0;
}
```

And similarly for `operator=`.

**Step 11: Fix CopySlots()**

Since `slots_` is now a pointer, CopySlots works the same but references `slots_[i]` via pointer. No change needed to the loop body — only remove the old `CopySlots` call logic and replace with a memcpy or keep as-is since the pointer dereference is equivalent.

**Step 12: Fix ProcessCompletions() and FindSlotByDescHead()**

These already access `slots_[i]` via index — since `slots_` changed from `std::array` to raw pointer, `slots_[i]` still works. No changes needed.

**Step 13: Build to verify (will fail — caller not updated yet)**

Continue to Task 6.

---

### Task 6: Update VirtioDriver to construct DmaRegion and slot buffer

**Files:**
- Modify: `src/device/virtio/virtio_driver.hpp`
- Modify: `src/device/virtio/virtio_driver.cpp`

**Step 1: Add slot buffer pool to virtio_driver.hpp**

Add `#include "dma_region.hpp"` to includes. Add after `dma_buffers_`:

```cpp
std::array<etl::unique_ptr<IoBuffer>, kMaxBlkDevices> slot_buffers_;
```

Update `Remove()` to also reset slot buffers:

```cpp
auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
  for (size_t i = 0; i < blk_device_count_; ++i) {
    blk_devices_[i].reset();
    dma_buffers_[i].reset();
    slot_buffers_[i].reset();
  }
  blk_device_count_ = 0;
  blk_adapter_count_ = 0;
  return {};
}
```

**Step 2: Update VirtioDriver::Probe() in virtio_driver.cpp**

After allocating `dma_buffers_[idx]`, add slot buffer allocation:

```cpp
// Allocate slot DMA buffer
auto [slot_size, slot_align] =
    virtio::blk::VirtioBlk<>::GetRequiredSlotMemSize();
slot_buffers_[idx] = kstd::make_unique<IoBuffer>(slot_size);
if (!slot_buffers_[idx] || !slot_buffers_[idx]->IsValid()) {
  klog::err << "VirtioDriver: failed to allocate slot DMA buffer at "
            << klog::hex << ctx->base;
  dma_buffers_[idx].reset();
  return std::unexpected(Error(ErrorCode::kOutOfMemory));
}
```

Replace the `VirtioBlk::Create()` call:

```cpp
// FROM:
auto result = virtio::blk::VirtioBlk<>::Create(
    ctx->base, dma_buffers_[idx]->GetBuffer().data(), kDefaultQueueCount,
    kDefaultQueueSize, extra_features);

// TO:
auto vq_dma = dma_buffers_[idx]->ToDmaRegion();
auto slot_dma = slot_buffers_[idx]->ToDmaRegion();

auto result = virtio::blk::VirtioBlk<>::Create(
    ctx->base, vq_dma, slot_dma, IdentityVirtToPhys,
    kDefaultQueueCount, kDefaultQueueSize, extra_features);
```

**Step 3: Build and run all tests**

Run:
```bash
cd build_x86_64 && make unit-test
```

Expected: All tests PASS.

**Step 4: Commit Task 4 + 5 + 6 together**

These three tasks form one atomic refactor — commit together:

```bash
git add src/include/dma_region.hpp \
        src/device/virtio/virt_queue/split.hpp \
        src/device/virtio/device/blk/virtio_blk.hpp \
        src/device/virtio/virtio_driver.hpp \
        src/device/virtio/virtio_driver.cpp \
        src/include/io_buffer.hpp \
        src/io_buffer.cpp \
        tests/unit_test/mocks/io_buffer_mock.cpp
git commit --signoff -m "refactor(device): replace void* DMA with DmaRegion + VirtToPhys abstraction

- SplitVirtqueue accepts DmaRegion instead of void* + uint64_t
- VirtioBlk::Create() takes DmaRegion for virtqueue and slot memory
- RequestSlot array allocated from dedicated DMA region
- VirtToPhysFunc callback replaces 5 identity-mapping reinterpret_cast sites
- Identity mapping isolated to VirtioDriver::Probe()
- IoBuffer gains ToDmaRegion() convenience method"
```

---

### Task 7: Update existing VirtioDriver tests

**Files:**
- Modify: `tests/unit_test/virtio_driver_test.cpp`

**Step 1: Verify existing tests still pass**

Run:
```bash
cd build_x86_64 && make unit-test
```

The existing 3 tests (`GetEntryNameIsVirtio`, `MatchStaticReturnsFalseWhenNoMmioBase`, `MatchTableContainsVirtioMmio`) don't touch `Create()` or DMA, so they should pass without changes.

**Step 2: If all pass, commit (no changes needed)**

No commit needed if tests pass as-is.

---

### Task 8: Final verification

**Step 1: Run format check**

```bash
pre-commit run --all-files
```

**Step 2: Build for all architectures (if cross-compilers available)**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel
cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

**Step 3: Run unit tests one final time**

```bash
cd build_x86_64 && make unit-test
```

Expected: All tests PASS, zero regressions.

---

## Summary of changes

| Identity-mapping site | Before | After |
|---|---|---|
| `virtio_blk.hpp:151` | `reinterpret_cast<uintptr_t>(vq_dma_buf)` | Eliminated — `DmaRegion.phys` |
| `virtio_blk.hpp:318` | `reinterpret_cast<uintptr_t>(data)` | `virt_to_phys_(...)` |
| `virtio_blk.hpp:338` | `reinterpret_cast<uintptr_t>(const_cast<...>)` | `virt_to_phys_(...)` |
| `virtio_blk.hpp:576` | `reinterpret_cast<uintptr_t>(&slot.header)` | `slot_dma_.phys + offset` |
| `virtio_blk.hpp:590` | `reinterpret_cast<uintptr_t>(&slot.status)` | `slot_dma_.phys + offset` |

All identity-mapping is pushed to one site: `VirtioDriver::Probe()`, which constructs `DmaRegion` from `IoBuffer::ToDmaRegion(IdentityVirtToPhys)`.
