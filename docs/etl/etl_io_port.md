# ETL `io_port`：类型安全的内存映射 I/O

> **对应内核文件**：`src/include/io.hpp`
> **ETL 头文件**：`<etl/io_port.h>`（已在 `3rd/etl/include/etl/io_port.h`）

## 一、现有实现的问题

`src/include/io.hpp` 提供了两个模板函数：

```cpp
namespace io {
template <std::integral T>
static __always_inline auto In(const uint64_t addr) -> T {
  return *reinterpret_cast<volatile T*>(addr);
}

template <std::integral T>
static __always_inline void Out(const uint64_t addr, const T data) {
  *reinterpret_cast<volatile T*>(addr) = data;
}
}  // namespace io
```

这种设计的**主要缺陷**：

1. **无访问方向约束**：对只读寄存器（如状态寄存器）调用 `io::Out()` 是合法的 C++，但是硬件错误。
2. **地址类型不安全**：`uint64_t addr` 可以传任意整数，类型系统无法防止把错误的地址传给特定寄存器。
3. **无寄存器语义**：读写一个裸地址，代码看不出这是哪个外设的哪个寄存器。
4. **重复读写模式冗余**：每次读写都要传递地址，对同一组寄存器操作时需要反复传同一个基地址。

---

## 二、`etl::io_port` 的设计

ETL 提供四种 io_port 变体，在**编译期**强制访问方向：

| 类型 | 语义 | 读 | 写 |
|------|------|----|----|
| `etl::io_port_ro<T, Addr>` | Read-Only（只读）| ✅ | ❌（编译错误）|
| `etl::io_port_wo<T, Addr>` | Write-Only（只写）| ❌（编译错误）| ✅ |
| `etl::io_port_rw<T, Addr>` | Read-Write（读写）| ✅ | ✅ |
| `etl::io_port_wos<T, Addr>` | Write with Shadow（写+影子寄存器）| ✅（读影子）| ✅（写硬件）|

所有变体均实现为 `volatile` 指针操作，**零运行时开销**（地址作为模板参数时编译器直接内联为立即数）。

---

## 三、两种使用模式

### 模式 A：编译期固定地址（仅供参考）

将寄存器地址作为**模板非类型参数**，编译器直接生成等价于 `*reinterpret_cast<volatile T*>(Addr)` 的代码：

```cpp
#include "etl_profile.h"
#include <etl/io_port.h>

// NS16550A UART 寄存器（以 RISC-V QEMU virt 为例，基地址 0x10000000）
using Uart0Rbr = etl::io_port_ro<uint8_t, 0x10000000>;  // 接收缓冲寄存器（只读）
using Uart0Thr = etl::io_port_wo<uint8_t, 0x10000000>;  // 发送保持寄存器（只写）
using Uart0Lsr = etl::io_port_ro<uint8_t, 0x10000005>;  // 线路状态寄存器（只读）

// 写一个字节到串口
void UartPutChar(char c) {
  while (!(Uart0Lsr::read() & 0x20)) {}
  Uart0Thr::write(static_cast<uint8_t>(c));
}
```

> **SimpleKernel 不使用此模式**：内核通过 FDT 动态解析设备基地址，编译期无法确定，
> 故统一采用下方的模式 B。此处仅供理解 ETL 设计原理。

### 模式 B：运行时动态地址（**SimpleKernel 唯一使用的模式**）

内核通过 FDT 解析设备树获取 MMIO 基地址，此时地址在运行时才知道。使用运行时地址构造：

```cpp
#include "etl_profile.h"
#include <etl/io_port.h>

class Ns16550aDriver {
 public:
  // 从 FDT 获取基地址后构造
  explicit Ns16550aDriver(uintptr_t base)
      : rbr_(reinterpret_cast<volatile uint8_t*>(base + 0)),
        thr_(reinterpret_cast<volatile uint8_t*>(base + 0)),
        lsr_(reinterpret_cast<volatile uint8_t*>(base + 5)) {}

  void PutChar(char c) {
    while (!(lsr_.read() & 0x20)) {}
    thr_.write(static_cast<uint8_t>(c));
  }

 private:
  // 运行时地址：第二模板参数省略（默认 0），通过构造函数传入指针
  etl::io_port_ro<uint8_t> rbr_;
  etl::io_port_wo<uint8_t> thr_;
  etl::io_port_ro<uint8_t> lsr_;
};
```

> **注意**：运行时地址模式中，`T*` 直接传递给构造函数。`io_port_rw<T>` 内部存储一个指针，
> 存在一个指针大小的额外内存开销（相比编译期地址模式，但这对内核驱动不是瓶颈）。

> **注意**：运行时地址模式中，`T*` 直接传递给构造函数。`io_port_rw<T>` 内部存储一个指针，
> 存在一个指针大小的额外内存开销（相比编译期地址模式）。

---

## 四、与 `io::In` / `io::Out` 的等价对照

| `io.hpp` 写法 | `etl::io_port` 等价写法 |
|--------------|----------------------|
| `io::In<uint8_t>(base + 5)` | `etl::io_port_ro<uint8_t, Addr>::read()` 或 `lsr_.read()` |
| `io::Out<uint8_t>(base + 0, 'A')` | `etl::io_port_wo<uint8_t, Addr>::write('A')` 或 `thr_.write('A')` |
| 读写同一地址 `In` + `Out` | `etl::io_port_rw<uint32_t, Addr>` |

---

## 五、在驱动中结构化寄存器组

ETL `io_port` 的最大价值在于将一个外设的所有寄存器**集中声明为有意义的类型**，而不是散落的地址：

```cpp
#include "etl_profile.h"
#include <etl/io_port.h>

// NS16550A 完整寄存器映射（基地址 0x10000000）
// 这份定义就是"文档"——一眼能看出每个寄存器的读写方向
struct Ns16550aRegs {
  static constexpr uintptr_t kBase = 0x10000000;

  // DLAB=0 模式
  using Rbr = etl::io_port_ro<uint8_t, kBase + 0>;  ///< 接收缓冲（只读）
  using Thr = etl::io_port_wo<uint8_t, kBase + 0>;  ///< 发送保持（只写）
  using Ier = etl::io_port_rw<uint8_t, kBase + 1>;  ///< 中断使能（读写）
  using Iir = etl::io_port_ro<uint8_t, kBase + 2>;  ///< 中断标识（只读）
  using Fcr = etl::io_port_wo<uint8_t, kBase + 2>;  ///< FIFO 控制（只写）
  using Lcr = etl::io_port_rw<uint8_t, kBase + 3>;  ///< 线路控制（读写）
  using Mcr = etl::io_port_rw<uint8_t, kBase + 4>;  ///< Modem 控制（读写）
  using Lsr = etl::io_port_ro<uint8_t, kBase + 5>;  ///< 线路状态（只读）
  using Msr = etl::io_port_ro<uint8_t, kBase + 6>;  ///< Modem 状态（只读）
};

// 使用时：
void UartInit() {
  Ns16550aRegs::Lcr::write(0x80);           // 设置 DLAB=1（访问波特率锁存器）
  Ns16550aRegs::Ier::write(0x00);           // 禁用所有中断
  Ns16550aRegs::Fcr::write(0xC7);           // 使能 FIFO，清除接收/发送 FIFO
}
```

---

## 六、`io_port_wos`：写并跟踪（影子寄存器）

某些只写寄存器需要在代码中"读回"之前写入的值（例如修改某一位时需要先读整体值）。
`etl::io_port_wos` 在 RAM 中维护一份"影子"副本：

```cpp
// 假设 GPIO 输出寄存器是只写的（硬件不支持读回）
using GpioOut = etl::io_port_wos<uint32_t, 0x40020014>;

GpioOut gpio;
gpio.write(0x01);                     // 写入硬件，影子 = 0x01
uint32_t shadow = gpio.read();        // 读影子 = 0x01（不访问硬件）
gpio.write(shadow | 0x02);           // 修改 bit1，写入硬件
```

> **内核适用性**：当前 SimpleKernel 无此需求（RISC-V/AArch64 的 MMIO 寄存器一般都可读），
> 但 x86_64 某些 IO 端口（`out` 指令）是纯只写的，若未来添加 x86 IO 端口支持时可使用。

---

## 七、底层原理

ETL `io_port` 的核心实现极其简单，以 `io_port_rw` 为例（截自 `3rd/etl/include/etl/io_port.h`）：

```cpp
template <typename T, uintptr_t Address = 0>
class io_port_rw {
 public:
  using value_type    = T;
  using pointer       = volatile T*;
  using const_pointer = volatile const T*;

  // 读：通过 volatile 指针读硬件寄存器
  value_type read() const {
    return *reinterpret_cast<const_pointer>(Address);
  }

  // 写：通过 volatile 指针写硬件寄存器
  void write(value_type value) {
    *reinterpret_cast<pointer>(Address) = value;
  }

  // 隐式转换（等同于 read()）
  operator value_type() const { return read(); }

  // 赋值（等同于 write()）
  void operator=(value_type value) { write(value); }
};
```

`volatile` 关键字是关键：它告诉编译器**每次访问都必须生成实际的内存读写指令**，不可优化掉或重排序。这与 `io::In` / `io::Out` 的 `*reinterpret_cast<volatile T*>(addr)` 语义完全等价，只是 ETL 的版本通过类型系统额外增加了访问方向约束。

---

## 八、迁移决策

**决策**：全面迁移到 `etl::io_port`（模式 B），**删除 `io.hpp`**，包含架构层代码。

### 迁移范围

| 层次 | 现有代码 | 迁移目标 | 说明 |
|------|---------|---------|------|
| **驱动层** (`src/device/include/driver/`) | `io::In<T>(addr)` / `io::Out<T>(addr, val)` | `etl::io_port_ro/wo/rw<T>` 成员变量 | 按外设建立寄存器结构体，优先迁移 |
| **架构层** (`src/arch/{arch}/`) | `io::In<T>(addr)` / `io::Out<T>(addr, val)` | `etl::io_port_ro/wo/rw<T>` 成员变量 | 同样迁移，不保留 `io::In/Out` |
| **`io.hpp`** | 独立头文件 | **删除** | 全面迁移完成后删除，不保留 |

### 迁移步骤

1. **驱动层**：将每个驱动的寄存器访问重构为 `etl::io_port` 成员变量，使用模式 B（运行时地址）。
2. **架构层**：将 `src/arch/{arch}/` 中的 `io::In/Out` 调用逐一替换为 `etl::io_port` 成员变量或局部变量。
3. **删除 `io.hpp`**：迁移完成后，从代码库中删除 `src/include/io.hpp` 及所有 `#include "io.hpp"` 引用。

### 迁移示例（架构层）

```cpp
// 迁移前：io.hpp 写法
io::Out<uint32_t>(base_ + kCtrlOffset, ctrl_val);
auto status = io::In<uint32_t>(base_ + kStatusOffset);

// 迁移后：etl::io_port 写法（模式 B）
class SomeArchDevice {
 public:
  explicit SomeArchDevice(uintptr_t base)
      : ctrl_(reinterpret_cast<volatile uint32_t*>(base + kCtrlOffset)),
        status_(reinterpret_cast<volatile uint32_t*>(base + kStatusOffset)) {}

 private:
  static constexpr uint32_t kCtrlOffset   = 0x00;
  static constexpr uint32_t kStatusOffset = 0x04;
  etl::io_port_wo<uint32_t> ctrl_;    ///< 控制寄存器（只写）
  etl::io_port_ro<uint32_t> status_;  ///< 状态寄存器（只读）
};
```

---

## 九、限制

1. **`ETL_NO_CHECKS` 禁用了越界检查**：`io_port` 本身不涉及边界，此项无影响。
2. **所有地址均为运行时模式（模式 B）**：FDT 解析得到的地址是运行时值，统一使用运行时地址构造。
3. **不适用于 RISC-V SBI 调用**：SBI 调用通过 `ecall` 指令，不是 MMIO，不应使用 `io_port`。
4. **不适用于 x86 端口 I/O**（`in`/`out` 指令）：x86 IO 端口不是内存映射的，需要用 `cpu_io` 的 `inb`/`outb`。
