# MPMCQueue

一个适用于 freestanding 环境的 C++26 多生产者多消费者（MPMC）无锁队列实现。

A C++26 Multi-Producer Multi-Consumer (MPMC) lock-free queue implementation for freestanding environments.

## 特性 (Features)

- ✅ **无锁设计** - 使用原子操作实现真正的无锁并发
- ✅ **C++26 支持** - 使用最新的 C++26 标准特性
- ✅ **Freestanding 环境** - 不依赖动态内存分配、异常处理等标准库设施
- ✅ **高性能** - 基于环形缓冲区的实现，具有缓存行对齐优化
- ✅ **多生产者多消费者** - 支持任意数量的生产者和消费者线程
- ✅ **Header-only** - 仅头文件库，易于集成
- ✅ **类型安全** - 完全的模板化实现，支持任意类型
- ✅ **STL 风格接口** - 提供类似于 STL 容器的接口

---

- ✅ **Lock-free design** - True lock-free concurrency using atomic operations
- ✅ **C++26 support** - Uses latest C++26 standard features
- ✅ **Freestanding environment** - No dependency on dynamic memory allocation, exceptions, etc.
- ✅ **High performance** - Ring buffer based implementation with cache line alignment
- ✅ **Multi-producer multi-consumer** - Supports any number of producer and consumer threads
- ✅ **Header-only** - Easy to integrate
- ✅ **Type-safe** - Fully templated implementation supporting any type
- ✅ **STL-like Interface** - Provides interface similar to STL containers

## 要求 (Requirements)

- C++26 兼容的编译器
  - GCC 14+
  - Clang 18+
  - MSVC 2024+
- CMake 3.20+（用于构建示例和测试）

---

- C++26 compatible compiler
  - GCC 14+
  - Clang 18+
  - MSVC 2024+
- CMake 3.20+ (for building examples and tests)

## 使用方法 (Usage)

### 基本示例 (Basic Example)

```cpp
#include <MPMCQueue.hpp>

int main() {
    // 创建一个容量为 256 的队列（必须是 2 的幂）
    // Create a queue with capacity of 256 (must be power of 2)
    mpmc_queue::MPMCQueue<int, 256> queue;

    // 入队
    // Enqueue
    queue.push(42);
    queue.push(100);

    // 出队
    // Dequeue
    int value;
    if (queue.pop(value)) {
        // 成功获取值
        // Successfully got value
    }

    return 0;
}
```

### 多线程示例 (Multi-threaded Example)

```cpp
#include <MPMCQueue.hpp>
#include <thread>

mpmc_queue::MPMCQueue<int, 1024> queue;

// 生产者线程 (Producer thread)
void producer() {
    for (int i = 0; i < 10000; ++i) {
        while (!queue.push(i)) {
            std::this_thread::yield();
        }
    }
}

// 消费者线程 (Consumer thread)
void consumer() {
    int value;
    while (queue.pop(value)) {
        // 处理值
        // Process value
    }
}

int main() {
    std::thread t1(producer);
    std::thread t2(consumer);

    t1.join();
    t2.join();

    return 0;
}
```

## 构建 (Building)

### 使用 CMake 和 Presets (Using CMake and Presets)

```bash
# 配置 (Release)
# Configure (Release)
cmake --preset=release

# 构建
# Build
cmake --build build/release

# 运行测试
# Run tests
cd build/release && ctest

# 运行示例
# Run examples
./build/release/examples/basic_example
./build/release/examples/threaded_example
```

### 作为依赖项集成 (Integration as Dependency)

#### 选项 1: Header-only

只需将 `include/MPMCQueue.hpp` 复制到你的项目中。

Simply copy `include/MPMCQueue.hpp` to your project.

#### 选项 2: CMake 子项目 (CMake Subproject)

```cmake
add_subdirectory(MPMCQueue)
target_link_libraries(your_target PRIVATE mpmc_queue)
```

#### 选项 3: CMake 安装 (CMake Install)

```bash
cmake --preset=release
cmake --build build/release
sudo cmake --install build/release
```

然后在你的 CMakeLists.txt 中：
Then in your CMakeLists.txt:

```cmake
find_package(MPMCQueue REQUIRED)
target_link_libraries(your_target PRIVATE mpmc::mpmc_queue)
```

## API 文档 (API Documentation)

### MPMCQueue<T, Capacity>

主要的队列类模板。

Main queue class template.

**模板参数 (Template Parameters):**
- `T` - 队列中元素的类型 / Type of elements in the queue
- `Capacity` - 最大元素数量（必须是 2 的幂）/ Maximum number of elements (must be power of 2)

**类型定义 (Type Definitions):**
- `value_type`
- `size_type`
- `reference`
- `const_reference`

**成员函数 (Member Functions):**

#### `bool push(const T& item) noexcept`
#### `bool push(T&& item) noexcept`

尝试将元素入队。
Attempt to enqueue an item.

**返回 (Returns):** `true` 如果成功，`false` 如果队列已满
**Returns:** `true` if successful, `false` if queue is full

#### `bool pop(T& item) noexcept`

尝试将元素出队。
Attempt to dequeue an item.

**参数 (Parameters):** `item` - 用于存储出队元素的引用
**Parameters:** `item` - Reference to store the dequeued item

**返回 (Returns):** `true` 如果成功，`false` 如果队列为空
**Returns:** `true` if successful, `false` if queue is empty

#### `static constexpr size_t max_size() noexcept`

返回队列的容量。
Returns the capacity of the queue.

#### `size_t size() const noexcept`

返回队列的近似大小。注意：在并发场景下这只是一个近似值。
Returns approximate size of the queue. Note: This is approximate in concurrent scenarios.

#### `bool empty() const noexcept`

检查队列是否为空（近似）。注意：在并发场景下这只是一个近似值。
Check if queue is empty (approximate). Note: This is approximate in concurrent scenarios.

## 设计说明 (Design Notes)

### 无锁算法 (Lock-free Algorithm)

本实现基于 Dmitry Vyukov 的 MPMC 队列设计，使用：
This implementation is based on Dmitry Vyukov's MPMC queue design, using:

- 环形缓冲区存储数据 / Ring buffer for data storage
- 原子序列号跟踪单元状态 / Atomic sequence numbers to track cell states
- CAS（比较并交换）操作实现无锁 / CAS (Compare-And-Swap) operations for lock-free behavior
- 缓存行对齐避免伪共享 / Cache line alignment to avoid false sharing

### Freestanding 环境兼容性 (Freestanding Environment Compatibility)

本库设计用于 freestanding C++ 环境：
This library is designed for freestanding C++ environments:

- 不使用动态内存分配（malloc/new）/ No dynamic memory allocation (malloc/new)
- 不使用异常处理 / No exception handling
- 不依赖完整的标准库 / No dependency on full standard library
- 仅使用 `<atomic>`、`<cstddef>` 和 `<cstdint>` / Only uses `<atomic>`, `<cstddef>`, and `<cstdint>`

对于完整的 freestanding 模式，可以使用编译选项：
For full freestanding mode, use compile options:

```bash
cmake -B build -DMPMC_FREESTANDING=ON
```

或直接使用编译器标志：
Or use compiler flags directly:

```bash
g++ -std=c++26 -ffreestanding -fno-exceptions -fno-rtti your_code.cpp
```

## 性能考虑 (Performance Considerations)

- **容量选择** - 使用 2 的幂作为容量以优化取模运算
- **Capacity** - Use power of 2 for capacity to optimize modulo operations

- **缓存行对齐** - 数据结构已对齐以避免伪共享
- **Cache line alignment** - Data structures are aligned to avoid false sharing

- **重试策略** - 在高竞争场景下，考虑添加退避策略
- **Retry strategy** - Consider adding backoff strategy in high contention scenarios

## 许可证 (License)

MIT License - 详见 [LICENSE](LICENSE) 文件

MIT License - See [LICENSE](LICENSE) file for details

## 贡献 (Contributing)

欢迎贡献！请随时提交 Pull Request。

Contributions are welcome! Please feel free to submit a Pull Request.

## 致谢 (Acknowledgments)

基于 Dmitry Vyukov 的无锁队列设计。

Based on Dmitry Vyukov's lock-free queue design.
