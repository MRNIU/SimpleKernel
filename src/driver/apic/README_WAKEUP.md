# APIC 多核心唤醒使用指南

## 概述

本 APIC 驱动实现了完整的多核心处理器唤醒功能，支持通过标准的 INIT-SIPI-SIPI 序列唤醒应用处理器 (Application Processor, AP)。

## 核心功能

### LocalApic 类

- `WakeupAp(uint32_t destination_apic_id, uint8_t start_vector)`: 执行完整的 INIT-SIPI-SIPI 序列
- `SendInitIpi(uint32_t destination_apic_id)`: 发送 INIT IPI
- `SendStartupIpi(uint32_t destination_apic_id, uint8_t start_page)`: 发送 SIPI

### Apic 管理类

- `StartupAp(uint32_t apic_id, uint8_t start_vector)`: 启动单个 AP
- `StartupAllAps(uint8_t start_vector, uint32_t max_wait_ms)`: 批量启动所有 AP
- `SetCpuOnline(uint32_t apic_id)`: 标记 CPU 为在线状态
- `IsCpuOnline(uint32_t apic_id)`: 检查 CPU 是否在线

## 使用流程

### 1. 准备工作

```cpp
// 初始化 APIC 系统
Apic apic_manager;
if (!apic_manager.Init(max_cpu_count)) {
    klog::Err("Failed to initialize APIC system\\n");
    return;
}

// 初始化当前 BSP 的 Local APIC
if (!apic_manager.InitCurrentCpuLocalApic()) {
    klog::Err("Failed to initialize BSP Local APIC\\n");
    return;
}
```

### 2. 准备 AP 启动代码

AP 启动代码必须满足以下要求：

- 位于 1MB 以下的物理内存中
- 16 位实模式代码
- 起始地址必须是 4KB 对齐的

```cpp
// 假设 AP 启动代码位于物理地址 0x8000
uint8_t ap_start_vector = 0x08;  // 0x8000 / 4096 = 0x08
```

### 3. 启动 AP

#### 方法一：启动单个 AP

```cpp
uint32_t target_apic_id = 1;
if (apic_manager.StartupAp(target_apic_id, ap_start_vector)) {
    klog::Info("AP startup sequence sent successfully\\n");
}
```

#### 方法二：批量启动所有 AP（推荐）

```cpp
size_t started_aps = apic_manager.StartupAllAps(ap_start_vector, 5000);
klog::Info("Started %zu APs\\n", started_aps);
```

### 4. AP 自我报告

每个 AP 在启动完成后应该调用以下函数报告其状态：

```cpp
// 在 AP 的启动代码中
apic_manager.SetCpuOnline(GetCurrentApicId());
```

### 5. 检查状态

```cpp
// 检查在线 CPU 数量
klog::Info("Total online CPUs: %zu\\n", apic_manager.GetCpuCount());

// 列出所有在线 CPU
for (size_t i = 0; i < apic_manager.GetCpuCount(); ++i) {
    uint32_t apic_id = apic_manager.GetOnlineCpus()[i];
    klog::Info("CPU %zu: APIC ID 0x%x\\n", i, apic_id);
}
```

## INIT-SIPI-SIPI 序列说明

标准的 AP 唤醒序列包含以下步骤：

1. **INIT IPI**: 将目标 AP 置于已知状态
2. **等待 10ms**: 让 AP 完成初始化
3. **第一个 SIPI**: 发送启动地址
4. **等待 200μs**: 短暂等待
5. **第二个 SIPI**: 重复发送以提高可靠性
6. **等待 200μs**: 最终等待

## 注意事项

### 1. 启动代码要求

- AP 启动代码必须是 16 位实模式代码
- 起始地址必须在 1MB 以下且 4KB 对齐
- 代码需要处理从实模式到保护模式（或长模式）的转换

### 2. 同步考虑

- INIT-SIPI-SIPI 序列是异步的
- AP 的实际启动状态需要通过其他机制确认
- 建议使用共享内存变量进行状态同步

### 3. 错误处理

- 并非所有 APIC ID 都对应实际的 CPU 核心
- 某些 AP 可能启动失败，需要适当的错误处理
- 超时机制确保系统不会无限等待

### 4. 调试信息

所有关键操作都会输出详细的日志信息，便于调试：

```
Waking up AP with APIC ID 0x1, start vector 0x8
Step 1: Sending INIT IPI to APIC ID 0x1
Step 2: Sending first SIPI to APIC ID 0x1  
Step 3: Sending second SIPI to APIC ID 0x1
INIT-SIPI-SIPI sequence completed for APIC ID 0x1
```

## 示例代码

完整的使用示例请参考 `apic.cpp` 文件末尾的注释部分。

## 支持的模式

- **x2APIC 模式**: 使用 MSR 接口，支持更多 CPU 和更高性能
- **xAPIC 模式**: 使用内存映射接口，兼容性更好

驱动会自动检测并选择最合适的模式。
