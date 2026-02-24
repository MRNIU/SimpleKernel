# IoBuffer Module Design

## Overview
Currently, `VirtioBlkDriver` uses a fixed-size `dma_buf_[32768]` directly embedded within its class definition for IO handling. This limits reusability, inflates the driver object size, and hardcodes the memory allocation policy.

This design document outlines extracting the IO buffer into an independent, dynamically allocated RAII wrapper (`IoBuffer`), decoupled from specific drivers, and externally injected into drivers via the `DeviceNode` structure.

## Core Components

### 1. `IoBuffer` Class (RAII Wrapper)
A new module responsible for safely allocating and freeing aligned memory.
- **Location**: `src/include/io_buffer.hpp` (interface) and `src/io_buffer.cpp` (implementation).
- **Behavior**:
  - `IoBuffer(size_t size, size_t alignment = 4096)`: Allocates aligned memory using `aligned_alloc`.
  - `~IoBuffer()`: Safely frees memory using `aligned_free`.
  - Copy semantics: Disabled (`= delete`) to prevent double-free issues.
  - Move semantics: Supported (`= default` or custom move constructor) to allow ownership transfer.
  - Exposes `.data()` (returns `uint8_t*` or `void*`) and `.size()`.

### 2. `DeviceNode` Modification
The `DeviceNode` structure serves as the medium for passing hardware and resource information to drivers during `Probe()`.
- **Location**: `src/device/include/device_node.hpp`
- **Changes**:
  - Introduce an `IoBuffer* dma_buffer` (or similar pointer/unique_ptr, keeping pointer to avoid copying or moving the `DeviceNode` unexpectedly) to the `DeviceNode` struct.
  - This allows an external manager (like the platform bus or memory initializer) to allocate an `IoBuffer` and assign it to the device before the driver's `Probe()` is called.

### 3. `VirtioBlkDriver` Modification
- **Location**: `src/device/include/driver/virtio_blk_driver.hpp`
- **Changes**:
  - Remove the `alignas(4096) uint8_t dma_buf_[32768]{};` array.
  - During `Probe(DeviceNode& node)`, retrieve the dynamically allocated IO buffer from `node.dma_buffer`.
  - If `node.dma_buffer` is `nullptr` or invalid, `Probe` fails with a clear error.
  - Pass the external buffer pointer to `VirtioBlkType::Create()`.

## Memory Allocation Details
- SimpleKernel's memory allocator (via `bmalloc`) provides `aligned_alloc` and `aligned_free` in `<sk_stdlib.h>` or `<bmalloc.hpp>`.
- `IoBuffer` will internally rely on these standard kernel interfaces, abstracting away the explicit requirement of `aligned_free`.

## Benefits
- **Decoupling**: Drivers do not manage memory allocation manually.
- **Reusability**: Other devices (e.g., networking, generic block devices) can use the same `IoBuffer` class.
- **Safety**: RAII semantics prevent memory leaks and dangling pointers.
