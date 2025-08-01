# APIC 驱动

这个目录包含了 Advanced Programmable Interrupt Controller (APIC) 的驱动实现，专为多核系统设计。

## 架构设计

### 多核系统中的 APIC 结构

在多核系统中，APIC 系统包含：

1. **Local APIC**: 每个 CPU 核心都有一个 Local APIC
   - 支持传统 xAPIC 和现代 x2APIC 模式
   - 优先使用 x2APIC，不可用时自动回退到 xAPIC
   - 处理本地中断、IPI、定时器
   - xAPIC 通过内存映射访问，x2APIC 通过 MSR 访问

2. **IO APIC**: 系统级别的中断控制器
   - 通常有 1-2 个 IO APIC
   - 处理外部设备中断并路由到不同 CPU
   - 通过内存映射 I/O 访问

### 支持的 APIC 模式

#### x2APIC 模式 (推荐)
- 使用 MSR 接口访问
- 支持更多 CPU（最多 2^32 个）
- 更高的性能
- 需要 CPU 硬件支持

#### xAPIC 模式 (兼容性)
- 使用内存映射接口访问
- 支持最多 256 个 CPU
- 兼容旧系统
- 所有现代 CPU 都支持

驱动会自动检测 CPU 能力并选择最佳模式。

### 类结构

1. **LocalApic** - 管理 Local APIC 功能（per-CPU）
   - 自动模式检测（x2APIC 优先，回退到 xAPIC）
   - 处理器间中断 (IPI)
   - 定时器功能
   - 中断优先级管理
   - CPU 启动支持（INIT/SIPI）

2. **IoApic** - 管理 IO APIC 功能（系统级）
   - IRQ 重定向
   - 中断屏蔽/取消屏蔽
   - 重定向表管理

3. **Apic** - 多核系统管理类
   - 管理多个 CPU 的 Local APIC
   - 管理多个 IO APIC 实例
   - CPU 在线状态管理
   - AP (Application Processor) 启动

### 使用方式

```cpp
#include "apic.h"
#include "singleton.hpp"

// 通过单例访问 APIC 管理器
auto& apic = Singleton<Apic>::GetInstance();

// 初始化 APIC 系统（在 BSP 上执行）
if (!apic.Init(max_cpu_count)) {
    // 处理初始化失败
}

// 添加 IO APIC（通过 ACPI 或其他方式发现）
if (!apic.AddIoApic(io_apic_base_address, gsi_base)) {
    // 处理添加失败
}

// 在每个 CPU 核心上初始化 Local APIC
if (!apic.InitCurrentCpuLocalApic()) {
    // 处理初始化失败
}

// 获取当前 CPU 的 Local APIC
auto& local_apic = apic.GetCurrentLocalApic();

// 发送 EOI
local_apic.SendEoi();

// 发送 IPI 到指定 CPU
apic.SendIpi(target_apic_id, 0x30);

// 启动 AP
apic.StartupAp(ap_apic_id, start_vector);

// 设置 IRQ 重定向
apic.SetIrqRedirection(1, 0x21, target_apic_id); // 键盘中断

// 标记 CPU 为在线
apic.SetCpuOnline(apic.GetCurrentApicId());
```

## 特性

### Local APIC (per-CPU)
- ✅ x2APIC 模式支持
- ✅ 处理器间中断 (IPI)
- ✅ 定时器功能（周期性和单次）
- ✅ 任务优先级管理
- ✅ CPU 启动支持 (INIT/SIPI)

### IO APIC (系统级)
- ✅ IRQ 重定向
- ✅ 中断屏蔽控制
- ✅ 多 IO APIC 支持
- ✅ GSI (Global System Interrupt) 管理
- ✅ 重定向表管理

### 多核系统管理
- ✅ 支持最多 256 个 CPU 核心
- ✅ CPU 在线状态管理
- ✅ AP 启动序列
- ✅ IPI 广播和单播
- ✅ 多 IO APIC 管理（最多 8 个）

## 多核系统工作流程

### 1. 系统启动阶段（BSP）
```cpp
auto& apic = Singleton<Apic>::GetInstance();

// 1. 初始化 APIC 系统
apic.Init(detected_cpu_count);

// 2. 通过 ACPI 发现并添加 IO APIC
for (auto& io_apic_entry : acpi_io_apics) {
    apic.AddIoApic(io_apic_entry.base, io_apic_entry.gsi_base);
}

// 3. 初始化 BSP 的 Local APIC
apic.InitCurrentCpuLocalApic();
apic.SetCpuOnline(apic.GetCurrentApicId());
```

### 2. AP 启动阶段
```cpp
// 在 BSP 上启动 AP
for (auto apic_id : ap_list) {
    apic.StartupAp(apic_id, ap_startup_vector);
}

// 在每个 AP 上执行
void ap_main() {
    auto& apic = Singleton<Apic>::GetInstance();
    apic.InitCurrentCpuLocalApic();
    apic.SetCpuOnline(apic.GetCurrentApicId());
}
```

### 3. 运行时中断管理
```cpp
// 设置设备中断路由
apic.SetIrqRedirection(keyboard_irq, KEYBOARD_VECTOR, target_cpu);
apic.SetIrqRedirection(timer_irq, TIMER_VECTOR, target_cpu);

// 发送 IPI 通知其他 CPU
apic.SendIpi(target_cpu, IPI_VECTOR);
apic.BroadcastIpi(BROADCAST_VECTOR);
```

## 限制

- 仅支持 x2APIC 模式
- 不支持传统 xAPIC 模式
- 最多支持 256 个 CPU 核心
- 最多支持 8 个 IO APIC
- 当前实现为接口定义，具体功能需要实现

## 文件结构

```
apic/
├── include/
│   └── apic.h          # 头文件，包含所有类定义
├── apic.cpp            # Apic 多核管理类实现
├── local_apic.cpp      # LocalApic 类实现
├── io_apic.cpp         # IoApic 类实现
├── CMakeLists.txt      # 构建配置
└── README.md           # 说明文档
```

## 依赖

- cpu_io 库（用于 MSR 操作）
- kernel_log.hpp（用于日志输出）
- singleton.hpp（用于单例模式）

## 注意事项

1. Local APIC 操作是 per-CPU 的，每个 CPU 访问自己的 Local APIC
2. IO APIC 是系统级别的，多个 CPU 可能同时访问，需要考虑同步
3. 使用 klog 进行日志输出
4. 使用 std::array 而不是 std::vector
5. 所有接口目前都是空实现，需要根据具体需求填充
6. 支持 GSI 到 IRQ 的映射管理
7. CPU 在线状态管理用于多核协调
