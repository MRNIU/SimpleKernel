# SimpleKernel 接口层重构 TODO

> **目标**：将 SimpleKernel 转型为"面向 AI 的 OS learning 项目"。第一步是确保所有模块都有清晰的、与实现分离的接口层（.h/.hpp）。
>
> **原则**：
>
> - 头文件只保留类声明、纯虚接口、类型定义、常量；实现移到 `.cpp`
> - 同类设备驱动应有共同的抽象基类
> - 每个接口文件需要有完整的 Doxygen 注释，描述职责、前置条件、后置条件
> - 现有代码作为**参考实现**保留，不改变功能行为

---

## 🔴 高优先级

### TODO-1: 新增 `ConsoleDriver` 抽象基类

**背景**：`Ns16550a`（RISC-V/x86_64 串口）和 `Pl011`（AArch64 串口）都实现了相同的接口（`PutChar`/`GetChar`/`TryGetChar`），但没有共同的基类。AI 无法从接口层知道"串口驱动需要实现哪些方法"。

**操作**：

1. 创建 `src/driver/include/console_driver.h`
2. 定义纯虚基类 `ConsoleDriver`，包含以下纯虚方法：

    ```cpp
    class ConsoleDriver {
    public:
      virtual ~ConsoleDriver() = default;
      /// 输出一个字符
      virtual void PutChar(uint8_t c) const = 0;
      /// 阻塞式读取一个字符
      [[nodiscard]] virtual auto GetChar() const -> uint8_t = 0;
      /// 非阻塞式尝试读取一个字符，无数据返回 -1（实际为 0xFF）
      [[nodiscard]] virtual auto TryGetChar() const -> uint8_t = 0;
    };
    ```
3. 修改 `src/driver/ns16550a/include/ns16550a.h`，让 `Ns16550a` 继承 `ConsoleDriver`，方法加 `override`
4. 修改 `src/driver/pl011/include/pl011.h`，让 `Pl011` 继承 `ConsoleDriver`，方法加 `override`
5. 确保三个架构的编译都通过

**涉及文件**：

- 新建：`src/driver/include/console_driver.h`
- 修改：`src/driver/ns16550a/include/ns16550a.h`
- 修改：`src/driver/pl011/include/pl011.h`

---

### TODO-2: 新增 `early_console` 接口声明

**背景**：三个架构各自在 `early_console.cpp` 中用匿名命名空间实现早期控制台初始化（设置 `sk_putchar` 回调），没有头文件声明。AI 不知道这个模块的存在和职责。

**当前实现方式**：

- `src/arch/riscv64/early_console.cpp`：通过 OpenSBI 的 `sbi_debug_console_write_byte` 设置 `sk_putchar`
- `src/arch/aarch64/early_console.cpp`：通过直接写 PL011 寄存器设置 `sk_putchar`
- `src/arch/x86_64/early_console.cpp`：通过直接写 NS16550A 寄存器设置 `sk_putchar`

三者都使用了相同的模式：在匿名命名空间中定义 `EarlyConsole` 结构体，在其构造函数中设置 `sk_putchar` 全局函数指针，然后定义一个全局静态实例触发构造。

**操作**：

- 方案 A（推荐）：在 `src/arch/arch.h` 中添加注释块说明 early_console 的契约：

  ```cpp
  /**
   * @brief 早期控制台初始化（由各架构在 early_console.cpp 中通过全局构造函数自动完成）
   *
   * 契约：
   * - 在 _start 之前（全局构造阶段），必须设置 sk_putchar 函数指针（定义在 sk_stdio.h）
   * - 设置后 sk_printf / klog 即可用于早期调试输出
   * - 不需要堆、不需要中断、不需要页表
   *
   * 各架构实现参考：
   * - riscv64: 通过 OpenSBI ecall (sbi_debug_console_write_byte)
   * - aarch64: 直接写 PL011 MMIO 寄存器
   * - x86_64: 直接写 NS16550A IO 端口
   */
  ```
- 方案 B（可选，更显式）：创建 `src/arch/include/early_console.h`，声明一个显式的初始化函数 `void EarlyConsoleInit()`，然后修改三个架构的实现导出此函数，在 `_start` 中调用。这需要修改 `boot.S` 或 `arch_main.cpp`，改动较大。

**涉及文件**：

- 方案 A：修改 `src/arch/arch.h`（添加注释）
- 方案 B：新建 `src/arch/include/early_console.h`，修改三个架构的 `early_console.cpp`

---

### TODO-3: 重新设计 `driver.h`

**背景**：当前 `src/driver/include/driver.h` 只声明了一个 `Driver()` 函数，且 `src/driver/driver.cpp` 的实现是空死循环，没有实际功能。

**操作**：

- 评估是否需要驱动子系统入口。当前各驱动（串口、中断控制器）都是在 `arch_main.cpp` 中按架构直接初始化的
- 如果保留 `driver.h`：重新定义其职责，例如作为驱动注册/查找的接口
- 如果不需要：移除 `driver.h` 和 `driver.cpp`，在文档中说明驱动由各架构初始化代码直接管理
- **建议**：至少将 `Driver()` 的签名和注释更新为有意义的内容，或者直接删除

**涉及文件**：

- `src/driver/include/driver.h`
- `src/driver/driver.cpp`
- `src/driver/CMakeLists.txt`（如果删除的话）

---

## 🟡 中优先级

### TODO-4: `VirtualMemory` 接口与实现分离

**背景**：`src/include/virtual_memory.hpp` 有 424 行，所有方法（`MapPage`、`UnmapPage`、`FindPageTableEntry`、`MapMMIO`、`GetMapping`、`DestroyPageDirectory`、`ClonePageDirectory`、`RecursiveFreePageTable`、`RecursiveClonePageTable`）全部内联实现在头文件中。这使得 AI 看到头文件就看到了全部答案。

**操作**：

1. 在 `src/include/virtual_memory.hpp` 中只保留：

    - 类声明
    - public/private 方法签名
    - 成员变量定义
    - 常量定义
    - Doxygen 注释
2. 新建 `src/memory/virtual_memory.cpp`（或放在 `src/` 目录下，与 `memory.cpp` 同级），将所有方法实现移过去
3. 注意：构造函数中调用了 `MapMMIO`，移动实现时需要保持调用顺序
4. 更新 `src/CMakeLists.txt` 添加新的源文件

**涉及文件**：

- 修改：`src/include/virtual_memory.hpp`（剥离实现，只留声明）
- 新建：`src/virtual_memory.cpp`（或合适的位置）
- 修改：`src/CMakeLists.txt`

---

### TODO-5: `KernelFdt` 接口与实现分离

**背景**：`src/include/kernel_fdt.hpp` 有 548 行，所有 FDT 解析逻辑全部在头文件中内联实现。

**操作**：

1. 在 `src/include/kernel_fdt.hpp` 中只保留类声明和方法签名：

    - `GetCoreCount()`, `CheckPSCI()`, `GetMemory()`, `GetSerial()`, `GetTimebaseFrequency()` 等 public 方法
    - `FindNode()`, `GetRegProperty()`, `GetPsciMethod()` 等 private 方法的签名
    - 成员变量 `fdt_header_`
2. 新建 `src/kernel_fdt.cpp`，移入所有实现
3. 更新 CMakeLists.txt

**涉及文件**：

- 修改：`src/include/kernel_fdt.hpp`
- 新建：`src/kernel_fdt.cpp`
- 修改：`src/CMakeLists.txt`

---

### TODO-6: `KernelElf` 接口与实现分离

**背景**：`src/include/kernel_elf.hpp` 有 158 行，构造函数中的完整 ELF 解析逻辑在头文件中。

**操作**：

1. 头文件只保留：类声明、`CheckElfIdentity()` 等方法签名、成员变量（`symtab_`, `strtab_`, `ehdr_`, `phdr_`, `shdr_`, `elf_`）
2. 新建 `src/kernel_elf.cpp`，移入构造函数实现和各方法实现
3. 更新 CMakeLists.txt

**涉及文件**：

- 修改：`src/include/kernel_elf.hpp`
- 新建：`src/kernel_elf.cpp`
- 修改：`src/CMakeLists.txt`

---

### TODO-7: 调度器实现从头文件剥离到 `.cpp`

**背景**：四个调度器全部 header-only 实现：

- `src/task/include/cfs_scheduler.hpp`（219 行）
- `src/task/include/fifo_scheduler.hpp`（约 100 行）
- `src/task/include/rr_scheduler.hpp`（124 行）
- `src/task/include/idle_scheduler.hpp`（118 行）

基类 `scheduler_base.hpp`（153 行）已经是纯虚接口，这是好的。

**操作**：
对每个调度器：

1. 头文件只保留类声明（继承关系、方法签名 + override、私有成员变量）
2. 新建对应 `.cpp` 文件：

    - `src/task/cfs_scheduler.cpp`
    - `src/task/fifo_scheduler.cpp`
    - `src/task/rr_scheduler.cpp`
    - `src/task/idle_scheduler.cpp`
3. 移入所有方法实现
4. 更新 `src/task/CMakeLists.txt`

**注意**：当前 `src/task/` 目录下已有 `block.cpp`, `clone.cpp`, `exit.cpp`, `schedule.cpp`, `sleep.cpp`, `task_control_block.cpp`, `task_manager.cpp`, `tick_update.cpp`, `wait.cpp`, `wakeup.cpp`，说明任务管理器本身已经做了接口与实现分离，只是调度器没有。

**涉及文件**：

- 修改：`src/task/include/cfs_scheduler.hpp`, `fifo_scheduler.hpp`, `rr_scheduler.hpp`, `idle_scheduler.hpp`
- 新建：`src/task/cfs_scheduler.cpp`, `fifo_scheduler.cpp`, `rr_scheduler.cpp`, `idle_scheduler.cpp`
- 修改：`src/task/CMakeLists.txt`

---

### TODO-8: `SpinLock` 和 `Mutex` 实现从头文件剥离

**背景**：

- `src/include/spinlock.hpp`（160 行）：`Lock()`/`UnLock()` 方法标记为 `__always_inline`，完整实现在头文件中
- `src/include/mutex.hpp`（215 行）：`Lock()`/`UnLock()` 等完整实现在头文件中

**操作**：

- `SpinLock`：因为 `Lock()`/`UnLock()` 标记了 `__always_inline`，性能上需要内联。**建议保留 header-only，但添加更详细的 Doxygen 契约注释**（前置条件、后置条件、副作用）。如果要分离，需要移除 `__always_inline` 并测试性能影响
- `Mutex`：可以安全地将实现移到 `src/task/mutex.cpp`，头文件只留声明

**涉及文件**：

- `src/include/spinlock.hpp`（增强注释或保持不变）
- `src/include/mutex.hpp`（剥离实现）
- 新建：`src/task/mutex.cpp`（如果分离 Mutex）
- 修改：`src/task/CMakeLists.txt`

---

## 🟢 低优先级

### TODO-9: 考虑为定时器添加接口

**背景**：`TimerInit()` / `TimerInitSMP()` 在 `src/arch/arch.h` 中声明为自由函数，各架构在 `timer.cpp` 中实现。没有 `Timer` 类或接口。

**当前实现**：

- `src/arch/riscv64/timer.cpp`：通过 OpenSBI 的 `sbi_set_timer` 设置定时器，注册时钟中断处理函数调用 `TaskManager::TickUpdate()`
- `src/arch/aarch64/timer.cpp`：类似，通过 GIC 和系统寄存器
- `src/arch/x86_64/timer.cpp`：通过 Local APIC Timer

**操作**（可选）：

- 当前的自由函数声明在 `arch.h` 中已经足够清晰
- 如果要增加抽象，可以创建 `TimerDriver` 基类，但 OS 内核中定时器通常与架构强耦合，过度抽象可能不合适
- **建议**：在 `arch.h` 的 `TimerInit` 声明处添加详细的契约注释即可，说明：

  - 前置条件：中断系统已初始化（`InterruptInit` 已调用）
  - 后置条件：时钟中断以 `SIMPLEKERNEL_TICK` Hz 频率触发，每次触发调用 `TaskManager::TickUpdate()`
  - 依赖：`BasicInfo::interval`（时钟频率）

**涉及文件**：

- 修改：`src/arch/arch.h`（增强注释）

---

### TODO-10: 考虑为中断控制器驱动添加统一基类

**背景**：`Plic`（RISC-V）、`Gic`（AArch64）、`Apic`（x86_64）是三种不同的中断控制器驱动，各自独立，没有共同基类。

**分析**：

- 这三者的接口差异很大（PLIC 按 source_id/hart_id 管理，GIC 按 SGI/PPI/SPI 分类，APIC 分 Local/IO 两部分）
- 架构层已经通过 `InterruptBase`（`src/include/interrupt_base.h`）做了统一抽象
- 驱动层强制统一可能导致接口过于泛化或不自然

**建议**：暂不添加。当前的分层已经合理：

- 驱动层（`Plic`/`Gic`/`Apic`）：提供硬件特定的操作接口
- 架构层（`Interrupt : InterruptBase`）：封装驱动，提供统一的 `Do`/`RegisterInterruptFunc`/`SendIpi` 接口

如果未来需要，可以提取一个轻量级的 `InterruptControllerDriver` 接口，只定义 `Enable(irq)`/`Disable(irq)`/`Ack(irq)` 等最基础操作。

**涉及文件**：无（暂不操作）

---

## 📋 验证清单

完成以上 TODO 后，需要确保：

- [ ] `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel` 编译通过
- [ ] `cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel` 编译通过
- [ ] `cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel` 编译通过
- [ ] 三个架构 `make run` 功能行为不变
- [ ] 每个头文件只包含：类/函数声明、类型定义、常量、Doxygen 注释
- [ ] 每个头文件的 Doxygen 注释包含：职责描述、前置条件、后置条件、使用示例（可选）
- [ ] 所有新增/修改的文件符合 `.clang-format` 格式
