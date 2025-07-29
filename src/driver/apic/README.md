# APIC (Advanced Programmable Interrupt Controller)

APIC 是 **Advanced Programmable Interrupt Controller**（高级可编程中断控制器）的缩写，是 x86 架构中用于处理中断的现代化控制器系统。

## 简介

APIC 是对传统 8259A PIC（Programmable Interrupt Controller）的现代化替代品，专为多核/多处理器系统设计。它提供了更强大、更灵活的中断管理能力。

## APIC 的组成

APIC 系统主要由两个部分组成：

### 1. Local APIC（本地 APIC）
- **位置**：每个 CPU 核心内部都有一个 Local APIC
- **功能**：
  - 处理本地中断（如时钟中断、性能监控中断）
  - 接收来自 I/O APIC 的中断消息
  - 处理处理器间中断（IPI，Inter-Processor Interrupt）
  - 管理中断优先级和屏蔽
  - 提供定时器功能

### 2. I/O APIC（输入/输出 APIC）
- **位置**：位于主板芯片组中，独立于处理器
- **功能**：
  - 收集来自外部设备的中断信号
  - 将中断路由到适当的处理器
  - 支持中断重定向和负载均衡
  - 支持更多的中断源（通常 24 个引脚）

## APIC 相比 8259A PIC 的优势

### 1. 多处理器支持
- **8259A PIC**：只能与单个处理器通信
- **APIC**：天然支持多处理器系统，每个处理器都有自己的 Local APIC

### 2. 更多中断源
- **8259A PIC**：最多支持 15 个中断（两个 8259A 级联）
- **APIC**：I/O APIC 通常支持 24 个中断输入，可以有多个 I/O APIC

### 3. 更好的中断分发
- **8259A PIC**：中断只能发送给固定的处理器
- **APIC**：支持中断路由，可以动态选择目标处理器

### 4. 处理器间通信
- **8259A PIC**：不支持处理器间中断
- **APIC**：支持 IPI（Inter-Processor Interrupt），用于多核协调

### 5. 更精确的定时器
- **8259A PIC**：依赖外部定时器芯片
- **APIC**：Local APIC 内置高精度定时器

## APIC 的工作模式

### 1. 中断传递模式
- **Fixed**：固定模式，中断发送给指定的处理器
- **Lowest Priority**：发送给优先级最低的处理器
- **SMI**：系统管理中断
- **NMI**：不可屏蔽中断
- **INIT**：初始化信号
- **Startup**：启动信号

### 2. 触发模式
- **Edge Triggered**：边沿触发
- **Level Triggered**：电平触发

## APIC 编程接口

### Local APIC 寄存器（内存映射）
- **基地址**：通常位于 `0xFEE00000`
- **主要寄存器**：
  - `ICR`（Interrupt Command Register）：中断命令寄存器
  - `LVT`（Local Vector Table）：本地向量表
  - `TPR`（Task Priority Register）：任务优先级寄存器
  - `EOI`（End of Interrupt）：中断结束寄存器

### I/O APIC 寄存器
- **间接访问**：通过索引寄存器和数据寄存器访问
- **主要寄存器**：
  - `IOAPICID`：I/O APIC 标识符
  - `IOAPICVER`：版本信息
  - `IOREDTBL`：重定向表项

## 在 SimpleKernel 中的应用

在 SimpleKernel 项目中，APIC 驱动的作用是：

1. **替代传统 PIC**：从 8259A PIC 升级到现代 APIC 系统
2. **多核支持**：为未来的多核内核提供基础设施
3. **更好的中断管理**：提供更精确和灵活的中断处理
4. **高精度定时**：利用 Local APIC 定时器进行精确计时

## 相关资源

- Intel 64 and IA-32 Architectures Software Developer's Manual Volume 3A, Chapter 10
- ACPI 规范中关于 APIC 的描述
- 各种操作系统内核中的 APIC 实现（Linux、FreeBSD 等）

## 开发状态

APIC 驱动已经基本实现，包括以下功能：

### 已实现功能

1. **Local APIC 驱动**：
   - 初始化和配置
   - 支持 xAPIC 和 x2APIC 模式
   - 中断结束信号 (EOI)
   - 任务优先级管理
   - 处理器间中断 (IPI)
   - 本地定时器
   - APIC ID 获取

2. **I/O APIC 驱动**：
   - 检测和设置
   - 重定向表管理
   - 中断路由配置
   - IRQ 屏蔽/取消屏蔽

3. **APIC 管理器**：
   - 统一的系统初始化接口
   - 自动检测 APIC 支持
   - 禁用传统 8259A PIC
   - 简化的 API 接口

### 使用方法

#### 基本初始化

```cpp
#include "apic.h"

// 初始化 APIC 系统
if (!InitApic()) {
    // 处理初始化失败
    return;
}
```

#### 中断路由设置

```cpp
// 将 IRQ 1 (键盘) 路由到向量 33
RouteIrq(1, 33);

// 取消屏蔽该中断
auto& io_apic = GetIoApic(0);
io_apic.UnmaskIrq(1);
```

#### 定时器使用

```cpp
auto& local_apic = GetLocalApic();

// 启动周期定时器，向量 32，初始计数 1000000
local_apic.StartTimer(1000000, 32, true);

// 停止定时器
local_apic.StopTimer();
```

#### 中断处理

```cpp
void interrupt_handler() {
    // 处理中断逻辑...

    // 发送中断结束信号
    SendEoi();
}
```

### 文件结构

```
src/driver/apic/
├── include/
│   └── apic.h              # APIC 驱动头文件
├── apic.cpp                # Local APIC 和 I/O APIC 实现
├── apic_manager.cpp        # APIC 管理器和全局接口
├── example.cpp             # 使用示例
├── CMakeLists.txt          # 构建配置
└── README.md               # 说明文档
```

### 注意事项

1. **内存映射**：确保 APIC 基地址（通常 0xFEE00000）和 I/O APIC 基地址（通常 0xFEC00000）在页表中正确映射
2. **ACPI 集成**：当前 I/O APIC 发现使用简化实现，生产环境应从 ACPI MADT 表读取
3. **多处理器支持**：当前实现主要针对单核，多核支持需要额外的同步机制
4. **中断向量**：确保使用的中断向量号不与系统异常冲突（建议使用 32-255 范围）

### 依赖项

- **cpu_io**：MSR 访问和 CPUID 功能
- **内存管理**：APIC 寄存器的内存映射访问

### 未来改进

1. 从 ACPI MADT 表自动发现 I/O APIC
2. 支持多个 I/O APIC
3. 更完善的错误处理和日志记录
4. 多核启动支持（AP 启动）
5. 中断负载均衡
