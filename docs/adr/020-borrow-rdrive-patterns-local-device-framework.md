<!-- Copyright The SimpleKernel Contributors -->

# ADR-020: 借鉴 rdrive 思路而非直接引入为核心设备框架

> **状态**: 已接受
>
> **日期**: 2026-06-03
>
> **接受日期**: 2026-06-08
>
> **审计阶段**: R6 — 设备框架与驱动回看
>
> **涉及模块**: `src/device/`, `src/fs/fatfs_adapter.rs`, `crates/dma`, `crates/memory`

## 背景

SimpleKernel 当前设备层已经有一套很薄的本地模型：

- `src/device/mod.rs` 定义 `Device` / `DeviceType` 和 `device_init()` 门面；
- `src/device/manager.rs` 用 `Vec<Box<dyn Device>>` 保存已注册设备；
- `src/device/platform_bus.rs` 从 FDT 枚举 `virtio,mmio` 节点；
- `src/device/virtio.rs` 使用 `virtio-drivers` 初始化块设备，并通过 `virtio_blk()` 暴露给
  `src/fs/fatfs_adapter.rs` 和 `tests/device-test/`。

评估 `rcore-os/tgoskits` 的 `drivers/rdrive` 后，确认它提供了值得参考的动态驱动框架：

- `DriverRegister` 描述驱动名称、probe level、priority 和 probe kind；
- 支持 Static、FDT、ACPI、PCI probe 入口；
- 通过 `rdif-*` 抽象不同设备接口；
- 可使用 `.driver.register*` 链接段自动收集驱动注册表；
- 已发布为 `rdrive 0.20.1`，属于 `no_std` + `alloc` crate。

但 `rdrive` 同时引入一整套外部设备世界观：

- 设备句柄使用 `Arc` / `Weak`、裸指针和原子借用协议；
- 内部同步使用 `spin::Mutex` / `spin::RwLock`，不接入 SimpleKernel 的锁级别和
  `SpinLockIrq` 中断安全边界；
- MMIO 通过 `mmio-api` 的全局 `MmioOp`，与 SimpleKernel 的 `memory::MmioRegion`
  永久映射模型不一致；
- PCIe 路径引入 `pcie`、`rdif-pcie` 和额外 unsafe 边界；
- 自动注册需要链接脚本保留 `.driver.register*` 并暴露起止符号，当前 SimpleKernel 链接脚本尚无该段。

因此，本决策不是判断 `rdrive` 能否编译，而是判断它是否应成为 SimpleKernel 的核心设备框架。

## 备选方案

### 方案 A：直接引入 `rdrive` 作为核心设备框架

在 `src/device/device_init()` 中初始化 `rdrive`，用 `rdrive::register_add()` /
`rdrive::probe_all()` 取代当前 `DeviceManager` 和 `PlatformBus`。后续驱动实现
`rdif-*` 接口，并通过 `rdrive::Device<T>` 查找。

**优点**:

- 可以快速获得 FDT / PCIe / driver register / priority probe 的完整框架。
- 未来若大量复用 tgoskits / ArceOS 驱动，接口兼容成本较低。
- 已有 crates.io 发布版本，维护活动较近。

**缺点**:

- 第三方设备模型会成为 SimpleKernel 的核心架构边界。
- `spin::Mutex` / `RwLock`、裸指针句柄和手写 `Send` / `Sync` 需要重新审计是否适合
  SimpleKernel 的中断、调度和 SAS 模型。
- `mmio-api` 的全局 MMIO 操作入口会与 `memory::MmioRegion` 重叠。
- 自动注册需要修改链接脚本和启动注册流程。
- 现有 `virtio_blk()`、FAT 适配器和系统测试不能直接迁移，需要兼容层。

### 方案 B：借鉴 `rdrive` 的注册和 probe 思路，实现本地设备框架

保留 `crate::device` 作为 SimpleKernel 的设备门面，在本地实现更完整的驱动注册表：

- 本地 `DriverDescriptor` / `ProbeKind` / `ProbeLevel` / `ProbePriority`；
- FDT compatible 匹配与 phandle 到设备 ID 的映射；
- 本地 `BlockDevice` / `NetDevice` / `SerialDevice` 等接口 trait；
- 本地设备 registry 使用 SimpleKernel 的 `SpinLock` / `SpinLockIrq` 和锁级别规则；
- MMIO、DMA、FDT 和错误处理继续走 SimpleKernel 自己的 `memory` / `dma` / `fdt` / `DeviceError` 边界。

后续如需兼容 `rdif-*`，可以新增适配层，而不是让 `rdrive` 类型直接扩散到内核上层。

**优点**:

- 保持 SimpleKernel 设备层的教学性、可审计性和项目内一致性。
- 不把第三方 unsafe、锁模型和 MMIO 全局状态作为核心不变量。
- 可渐进迁移现有 `DeviceManager`，先保留 `virtio_blk()` / `device_count()` 等兼容门面。
- 未来仍可参考 `rdif-*` 或局部引入适配，不阻断驱动生态演进。

**缺点**:

- 需要自己实现注册表、probe 顺序、接口 trait 和测试。
- 短期无法直接复用完整 `rdrive` 驱动生态。
- 如果后续目标转向大量接入 tgoskits 驱动，可能需要再写兼容层或重新评估方案 A。

### 方案 C：保持当前薄设备模型，不做结构演进

继续使用 `Vec<Box<dyn Device>>`、手写 `platform_bus` 和 `virtio_blk()` 全局引用。

**优点**:

- 当前 QEMU VirtIO 块设备和 FAT 路径已经可运行。
- 改动最少，风险最低。

**缺点**:

- `Device` trait 只能表达名称和类型，不能表达块设备、网络、串口、DMA capability、
  中断父设备等真实接口契约。
- FDT 匹配逻辑会随着设备类型增加而变成集中式分发。
- 不利于 PCIe、NVMe、USB、设备树 phandle 依赖和 probe priority 演进。

## 决策

选择 **方案 B：借鉴 `rdrive` 的注册和 probe 思路，实现 SimpleKernel 本地设备框架**。

`rdrive` 不作为当前核心设备框架直接引入。SimpleKernel 后续应保留 `crate::device`
门面，并在其内部演进出本地 driver registry、probe descriptor 和设备接口 trait。

2026-06-08 收口后，本决策进入已接受状态。后续若要改变为直接引入 `rdrive`、让
`rdif-*` 成为上层公共接口，或采用 `mmio-api` / 自动链接段注册作为核心路径，必须重新
提交 ADR 或隔离 POC 结论。

## 理由

### 设备框架是内核核心边界，不只是普通依赖

普通 Rust crate 可以通过 Cargo 依赖和 feature 管理。但设备框架会决定：

- 驱动对象的所有权和生命周期；
- 中断上下文是否能访问设备；
- MMIO 映射和 DMA buffer 的安全边界；
- FDT / PCIe / platform bus 的错误处理策略；
- 文件系统、网络、syscall 等上层如何取得设备能力。

这些边界需要与 SimpleKernel 的 SAS 架构、锁级别、DMA wrapper 和 fail-fast 规则一致。

### `rdrive` 的思路可复用，但实现不应直接穿透项目边界

`rdrive` 的 descriptor + probe kind + priority 设计适合借鉴。它解决的是“驱动如何声明自己能匹配什么设备”。

但它的设备句柄、MMIO 全局入口、PCIe 枚举和锁模型是另一套内核环境下的实现选择。直接引入会让
SimpleKernel 的上层模块依赖外部类型，例如 `rdrive::Device<T>`、`rdif-*` 和 `mmio-api`。
这会削弱本项目当前“本地门面隔离第三方实现依赖”的设计习惯。

### 当前最需要的是演进本地接口，而不是换掉所有设备路径

当前明确依赖设备层的实际路径很少：

- `tests/device-test/` 验证设备计数和 VirtIO 块设备读扇区；
- `src/fs/fatfs_adapter.rs` 直接通过 `virtio_blk()` 做扇区读写；
- `src/smoke_test.rs` 只读取 `device_count()`。

这说明可以先把 `virtio_blk()` 迁移为更通用的 `BlockDevice` 门面，再替换内部 registry。
没有必要先引入完整 `rdrive` 再回头做兼容。

### 后续可以按条件重新评估

如果后续目标变为直接复用 tgoskits / ArceOS 的 PCIe、NVMe、USB 或 SoC 驱动，方案 A 的收益会提高。
届时应以一个隔离 POC 重新评估：

- `rdrive` 与 SimpleKernel 链接脚本、FDT 地址、MMIO 和 DMA 的适配成本；
- `rdif-*` 是否能覆盖目标驱动接口；
- `spin::Mutex` / `RwLock` 是否需要替换或包裹；
- QEMU 和真机 DMA / interrupt 行为是否可验证。

## 影响

- **代码变更**:
  - 短期不直接引入 `rdrive` 依赖。
  - 后续在 `src/device/` 内新增本地 driver descriptor、registry 和 typed device interface。
  - 逐步把当前 `DeviceManager` 从 `Vec<Box<dyn Device>>` 演进为能力接口 registry。
  - `src/device/virtio.rs` 继续保留 `virtio-drivers`，但应把块设备能力暴露为本地 `BlockDevice`。
- **API 变更**:
  - 第一阶段保留 `device_count()` 和 `virtio_blk()` 兼容入口。
  - 稳定后新增 `block_device()` 或等价门面，`src/fs/fatfs_adapter.rs` 不再直接依赖 VirtIO 具体类型。
  - 旧 `Device` / `DeviceType` 可在迁移完成后弃用。
- **测试**:
  - 保持 `device-test` 覆盖设备枚举和块设备读扇区。
  - 新增 registry 单元或系统测试，覆盖 probe priority、FDT compatible 匹配和重复注册。
  - 新增 `fs-test` 覆盖 FAT 通过 `BlockDevice` 门面读写。
- **文档**:
  - `docs/design/P6-设备框架与驱动.md` 同步记录本决策和后续演进方向。
  - `docs/design/device-subsystem-current.md` 记录当前 `src/device/` 设计边界和演进路径。
  - `docs/audit/2026-05-07-device-dma-rdrive-tracking.md` 保留 rdrive 评估入口，并更新为“借鉴优先”。
- **当前设计同步**:
  - `src/device/` 的当前 SDD 已从历史 P6 计划中拆出，见
    `docs/design/device-subsystem-current.md`。

## 参考

- [`rdrive` in `rcore-os/tgoskits`](https://github.com/rcore-os/tgoskits/tree/dev/drivers/rdrive)
  — 动态驱动管理、FDT/PCI probe 和驱动注册描述符参考。
- [`rdrive` crate](https://crates.io/crates/rdrive) — 已发布的 `no_std` + `alloc` 版本。
- [ADR-014: QEMU VirtIO DMA 抽象封装 `dma-api`](014-qemu-virtio-dma-api-wrapper.md)
  — 记录 SimpleKernel 对第三方 DMA crate 的本地门面策略。
- `docs/audit/2026-05-07-device-dma-rdrive-tracking.md` — 设备/DMA 与 rdrive 集成前检查入口。
