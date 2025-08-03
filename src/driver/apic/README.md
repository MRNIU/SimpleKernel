# APIC 驱动

这个目录包含了 Advanced Programmable Interrupt Controller (APIC) 的驱动实现，专为多核系统设计。

## 架构设计

### 多核系统中的 APIC 结构

在多核系统中，APIC 系统包含：

1. **Local APIC**: 每个 CPU 核心都有一个 Local APIC
   - ✅ 支持传统 xAPIC 和现代 x2APIC 模式
   - ✅ 优先使用 x2APIC，不可用时自动回退到 xAPIC
   - ✅ 处理本地中断、IPI、定时器
   - ✅ xAPIC 通过内存映射访问，x2APIC 通过 MSR 访问

2. **IO APIC**: 系统级别的中断控制器
   - ✅ 目前支持单个 IO APIC
   - ✅ 处理外部设备中断并路由到不同 CPU
   - ✅ 通过内存映射 I/O 访问

### 支持的 APIC 模式

#### x2APIC 模式 (推荐，已实现)
- ✅ 使用 MSR 接口访问
- ✅ 支持更多 CPU（最多 2^32 个）
- ✅ 更高的性能
- ✅ 自动检测 CPU 硬件支持

#### xAPIC 模式 (兼容性，已实现)
- ✅ 使用内存映射接口访问
- ✅ 支持最多 256 个 CPU
- ✅ 兼容旧系统
- ✅ 所有现代 CPU 都支持

驱动会自动检测 CPU 能力并选择最佳模式。

### 类结构

1. **LocalApic** - 管理 Local APIC 功能（per-CPU）
   - ✅ 自动模式检测（x2APIC 优先，回退到 xAPIC）
   - ✅ 处理器间中断 (IPI)
   - ✅ 定时器功能（周期性和单次）
   - ✅ 中断优先级管理
   - ✅ CPU 启动支持（INIT/SIPI/SIPI序列）
   - ✅ EOI (End of Interrupt) 支持
   - ✅ LVT (Local Vector Table) 配置

2. **IoApic** - 管理 IO APIC 功能（系统级）
   - ✅ IRQ 重定向到指定 CPU
   - ✅ 中断屏蔽/取消屏蔽
   - ✅ 重定向表管理
   - ✅ 自动初始化时屏蔽所有中断

3. **Apic** - 多核系统管理类
   - ✅ 管理当前 CPU 的 Local APIC
   - ✅ 管理单个 IO APIC 实例
   - ✅ AP (Application Processor) 启动
   - ✅ 禁用传统 8259A PIC

### 使用方式

```cpp
#include "apic.h"
#include "singleton.hpp"

// 通过单例访问 APIC 管理器
auto& apic = Singleton<Apic>::GetInstance();

// 初始化当前 CPU 的 Local APIC
if (!apic.InitCurrentCpuLocalApic()) {
    // 处理初始化失败
}

// 发送 EOI 信号
apic.SendEoi();

// 设置 Local APIC 定时器
apic.SetupPeriodicTimer(100, 0xF0);  // 100Hz, 中断向量 0xF0

// 发送 IPI 到指定 CPU
apic.SendIpi(target_apic_id, 0x30);

// 广播 IPI 到所有其他 CPU
apic.BroadcastIpi(0x31);

// 启动 AP
apic.StartupAp(ap_apic_id, ap_code_addr, ap_code_size, target_addr);

// 启动所有 AP
apic.StartupAllAps(ap_code_addr, ap_code_size, target_addr);

// 设置 IRQ 重定向
apic.SetIrqRedirection(1, 0x21, target_apic_id);  // 键盘中断

// 屏蔽/取消屏蔽 IRQ
apic.MaskIrq(1);    // 屏蔽键盘中断
apic.UnmaskIrq(1);  // 取消屏蔽键盘中断
```

## 已实现的特性

### Local APIC (per-CPU)
- ✅ x2APIC 模式支持（优先）
- ✅ xAPIC 模式支持（回退）
- ✅ 自动模式检测和切换
- ✅ 处理器间中断 (IPI)
- ✅ 定时器功能（周期性和单次）
- ✅ 任务优先级管理
- ✅ CPU 启动支持 (INIT/SIPI/SIPI 序列)
- ✅ EOI (End of Interrupt) 处理
- ✅ LVT (Local Vector Table) 配置
- ✅ 错误状态读取

### IO APIC (系统级)
- ✅ IRQ 重定向到指定 CPU
- ✅ 中断屏蔽/取消屏蔽控制
- ✅ 重定向表管理
- ✅ 自动初始化（屏蔽所有中断）
- ✅ 范围检查和错误处理

### 多核系统管理
- ✅ 支持任意数量的 CPU 核心
- ✅ AP 启动序列（单个和批量）
- ✅ IPI 广播和单播
- ✅ 传统 8259A PIC 禁用
- ✅ 完整的错误处理和日志

## 多核系统工作流程

### 1. 系统启动阶段（BSP）
```cpp
auto& apic = Singleton<Apic>::GetInstance();

// 1. 初始化当前 CPU 的 Local APIC
// 注意：构造函数会自动禁用传统 8259A PIC
apic.InitCurrentCpuLocalApic();
```

### 2. AP 启动阶段
```cpp
// 在 BSP 上启动单个 AP
apic.StartupAp(apic_id, ap_code_addr, ap_code_size, target_addr);

// 或者启动所有 AP
apic.StartupAllAps(ap_code_addr, ap_code_size, target_addr);

// 在每个 AP 上执行
void ap_main() {
    auto& apic = Singleton<Apic>::GetInstance();
    apic.InitCurrentCpuLocalApic();
}
```

### 3. 运行时中断管理
```cpp
// 设置设备中断路由
apic.SetIrqRedirection(1, 0x21, target_cpu);     // 键盘
apic.SetIrqRedirection(0, 0x20, target_cpu);     // 定时器

// 发送 IPI 通知其他 CPU
apic.SendIpi(target_cpu, 0x30);
apic.BroadcastIpi(0x31);

// 在中断处理函数中发送 EOI
apic.SendEoi();
```

## 当前实现状态

### ✅ 已完成的功能
- **Local APIC 完整实现**：支持 x2APIC/xAPIC 自动切换
- **IO APIC 基础功能**：IRQ 重定向和屏蔽控制
- **多核启动**：INIT/SIPI/SIPI 序列实现
- **中断管理**：IPI、定时器、EOI 处理
- **错误处理**：完整的边界检查和日志

### ⚠️ 当前限制
- 仅支持单个 IO APIC（大多数系统足够）
- 不支持 GSI (Global System Interrupt) 到 IRQ 的复杂映射
- 没有 CPU 在线状态管理（简化实现）

### 🔧 技术细节
- **内存安全**：使用 std::array 代替原始数组
- **模式检测**：运行时检测并选择最佳 APIC 模式
- **错误恢复**：初始化失败时的优雅降级

## 文件结构

```
apic/
├── include/
│   ├── apic.h          # APIC 管理类头文件
│   ├── local_apic.h    # Local APIC 类头文件
│   └── io_apic.h       # IO APIC 类头文件
├── apic.cpp            # APIC 管理类实现
├── local_apic.cpp      # Local APIC 类实现
├── io_apic.cpp         # IO APIC 类实现
├── CMakeLists.txt      # 构建配置
└── README.md           # 说明文档（本文件）
```

## 依赖

- **cpu_io 库**：MSR/PIC 操作、当前核心 ID 获取
- **kernel_log.hpp**：日志输出
- **singleton.hpp**：单例模式管理
- **io.hpp**：内存映射 I/O 操作

## 性能特点

1. **高效的模式选择**：优先使用 x2APIC 获得最佳性能
2. **最小化锁竞争**：Local APIC 操作无需同步
3. **完整的错误处理**：避免系统崩溃
4. **内存安全**：使用现代 C++ 特性

## 调试功能

```cpp
// 打印 APIC 信息用于调试
apic.PrintInfo();  // 打印整个 APIC 系统信息
```

输出示例：
```
Local APIC initialized successfully for CPU with APIC ID 0x0
IO APIC initialization completed
IO APIC Information
Base Address: 0xfec00000
ID: 0x0
Version: 0x20
Max Redirection Entries: 24
```
