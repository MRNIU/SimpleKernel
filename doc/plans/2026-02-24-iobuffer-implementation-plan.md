# IoBuffer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Decouple IO buffer memory management from driver implementations by creating a reusable, RAII-based `IoBuffer` class and passing it via `DeviceNode`.

**Architecture:** An `IoBuffer` class will wrap `aligned_alloc` and `aligned_free`. The `DeviceNode` structure will hold an `IoBuffer* dma_buffer` so external initializers can inject the buffer before device probe. `VirtioBlkDriver` will retrieve this buffer instead of using a static array.

**Tech Stack:** C++23, SimpleKernel Memory Allocators (`bmalloc`), GoogleTest (for host unit tests).

---

### Task 1: Create `IoBuffer` Header Interface

**Files:**
- Create: `src/include/io_buffer.hpp`

**Step 1: Write the interface definition**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_

#include <cstdint>
#include <cstddef>
#include <utility>

/**
 * @brief RAII wrapper for dynamically allocated, aligned IO buffers.
 *
 * @pre None
 * @post The buffer memory is correctly allocated and aligned. It will be freed upon destruction.
 */
class IoBuffer {
 public:
  /// Default alignment for IO buffers (e.g., page size)
  static constexpr size_t kDefaultAlignment = 4096;

  IoBuffer() = default;

  IoBuffer(size_t size, size_t alignment = kDefaultAlignment);

  ~IoBuffer();

  // Disable copy
  IoBuffer(const IoBuffer&) = delete;
  auto operator=(const IoBuffer&) -> IoBuffer& = delete;

  // Enable move
  IoBuffer(IoBuffer&& other) noexcept;
  auto operator=(IoBuffer&& other) noexcept -> IoBuffer&;

  [[nodiscard]] auto data() const -> uint8_t*;
  [[nodiscard]] auto size() const -> size_t;
  [[nodiscard]] auto is_valid() const -> bool;

 private:
  uint8_t* data_{nullptr};
  size_t size_{0};
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_IO_BUFFER_HPP_ */
```

**Step 2: Commit**

```bash
git add src/include/io_buffer.hpp
git commit -signoff -m "feat(memory): add IoBuffer interface"
```

---

### Task 2: Implement `IoBuffer` in C++ Source File

**Files:**
- Create: `src/io_buffer.cpp`
- Modify: `src/CMakeLists.txt` (to include `io_buffer.cpp`)

**Step 1: Write the implementation**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "io_buffer.hpp"

// SimpleKernel uses its own standard library headers
#include "sk_stdlib.h"
#include "kernel_log.hpp"

IoBuffer::IoBuffer(size_t size, size_t alignment) : size_(size) {
  if (size == 0 || alignment == 0 || (alignment & (alignment - 1)) != 0) {
    klog::Err("IoBuffer: invalid size (%lu) or alignment (%lu)\n", size, alignment);
    data_ = nullptr;
    return;
  }

  // Use aligned_alloc provided by bmalloc / sk_stdlib.h
  data_ = static_cast<uint8_t*>(aligned_alloc(alignment, size));
  if (data_ == nullptr) {
    klog::Err("IoBuffer: aligned_alloc failed for size %lu, alignment %lu\n", size, alignment);
  }
}

IoBuffer::~IoBuffer() {
  if (data_ != nullptr) {
    aligned_free(data_);
    data_ = nullptr;
  }
  size_ = 0;
}

IoBuffer::IoBuffer(IoBuffer&& other) noexcept : data_(other.data_), size_(other.size_) {
  other.data_ = nullptr;
  other.size_ = 0;
}

auto IoBuffer::operator=(IoBuffer&& other) noexcept -> IoBuffer& {
  if (this != &other) {
    if (data_ != nullptr) {
      aligned_free(data_);
    }
    data_ = other.data_;
    size_ = other.size_;
    other.data_ = nullptr;
    other.size_ = 0;
  }
  return *this;
}

auto IoBuffer::data() const -> uint8_t* { return data_; }

auto IoBuffer::size() const -> size_t { return size_; }

auto IoBuffer::is_valid() const -> bool { return data_ != nullptr; }
```

**Step 2: Add to `src/CMakeLists.txt`**

Find the `add_library(SimpleKernelSrc OBJECT` or similar section in `src/CMakeLists.txt` and append `io_buffer.cpp` to the sources.

**Step 3: Test Compilation**
Run: `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel`

**Step 4: Commit**

```bash
git add src/io_buffer.cpp src/CMakeLists.txt
git commit -signoff -m "feat(memory): implement IoBuffer"
```

---

### Task 3: Modify `DeviceNode` to carry `IoBuffer`

**Files:**
- Modify: `src/device/include/device_node.hpp`

**Step 1: Update DeviceNode structure**

```cpp
// Add include at the top
#include "io_buffer.hpp"

// Inside the DeviceNode struct, add:
// Note: We use a raw pointer to not complicate the copying/moving of DeviceNode.
// The external allocator retains ownership and lifetime responsibilities.
IoBuffer* dma_buffer{nullptr};
```

**Step 2: Commit**

```bash
git add src/device/include/device_node.hpp
git commit -signoff -m "feat(device): add dma_buffer pointer to DeviceNode"
```

---

### Task 4: Refactor `VirtioBlkDriver` to use `DeviceNode::dma_buffer`

**Files:**
- Modify: `src/device/include/driver/virtio_blk_driver.hpp`

**Step 1: Remove `dma_buf_` and use `node.dma_buffer`**

1. Remove `alignas(4096) uint8_t dma_buf_[32768]{};` from the `private:` section.
2. In `Probe(DeviceNode& node)`:

Replace:
```cpp
auto result = VirtioBlkType::Create(base, dma_buf_, 1, 128, extra_features);
```

With:
```cpp
if (node.dma_buffer == nullptr || !node.dma_buffer->is_valid()) {
  klog::Err("VirtioBlkDriver: Missing or invalid DMA buffer in DeviceNode at 0x%lX\n", base);
  node.bound.store(false, std::memory_order_release);
  return std::unexpected(Error(ErrorCode::kInvalidParam));
}

// Pass the pointer and size to Create (ensure the size meets virtio queue requirements)
auto result = VirtioBlkType::Create(base, node.dma_buffer->data(), 1, 128, extra_features);
```

**Step 2: Test Compilation**
Run: `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel`

**Step 3: Commit**

```bash
git add src/device/include/driver/virtio_blk_driver.hpp
git commit -signoff -m "refactor(device): virtio blk driver uses external IoBuffer from DeviceNode"
```

---

### Task 5: (Optional/Later) External memory allocation in Initialization
Since the system requires an initialized `IoBuffer` before the driver is probed, the code that populates `DeviceNode` for `virtio,mmio` devices (e.g., `kernel_fdt.cpp` or `platform_bus.hpp`) should allocate an `IoBuffer` and assign it to `node.dma_buffer` if it is a virtio block device. We will do a generic dynamic allocation fallback inside the device framework if not present for testing, or we leave it up to the board-specific initialization.

For this plan, the core requirement is met (extracting the IO buffer into an independent module and decoupling it).

**Step 1: Commit**
(No code changes needed here unless specifically asked to wire up the FDT parsing).
