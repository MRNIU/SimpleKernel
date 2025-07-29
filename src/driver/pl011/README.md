# PL011 UART 驱动

PL011 是 ARM 设计的通用异步接收器发送器 (UART)，广泛用于 aarch64 架构的串口通信。

## 功能特性

- 支持 8 位数据位，1 个停止位，无奇偶校验
- 支持可配置波特率
- 支持接收中断
- 支持 FIFO 缓冲
- 完整的寄存器级别控制

## 类接口

### 构造函数
```cpp
Pl011(uint64_t dev_addr, uint64_t clock = 0, uint64_t baud_rate = 0)
```
- `dev_addr`: 设备基地址
- `clock`: 串口时钟频率（可选）
- `baud_rate`: 波特率（可选）

### 主要方法
```cpp
void PutChar(uint8_t c)
```
发送单个字符到串口

## 使用示例

```cpp
// 在 aarch64 架构中的使用
auto [serial_base, serial_size, irq] =
    Singleton<KernelFdt>::GetInstance().GetSerial();

Singleton<Pl011>::GetInstance() = Pl011(serial_base);
pl011 = &Singleton<Pl011>::GetInstance();

// 发送字符
pl011->PutChar('H');
```

## 设备树配置示例

```dts
pl011@9000000 {
    clock-names = "uartclk\0apb_pclk";
    clocks = <0x8000 0x8000>;
    interrupts = <0x00 0x01 0x04>;
    reg = <0x00 0x9000000 0x00 0x1000>;
    compatible = "arm,pl011\0arm,primecell";
};
```

## 寄存器映射

驱动实现了完整的 PL011 寄存器映射，包括：
- 数据寄存器 (DR)
- 标志寄存器 (FR)
- 线控制寄存器 (LCRH)
- 控制寄存器 (CR)
- 波特率寄存器 (IBRD/FBRD)
- 中断相关寄存器等

## 参考文档

- [ARM PL011 技术参考手册](https://developer.arm.com/documentation/ddi0183/g/)

## 文件结构

- `include/pl011.h` - 头文件，包含类定义和寄存器常量
- `pl011.cpp` - 实现文件，包含构造函数和字符输出功能
- `CMakeLists.txt` - 构建配置
