// Copyright The SimpleKernel Contributors

//! 驱动 descriptor 与 probe 语义。

use crate::DeviceId;

/// 驱动 probe 函数。
pub type ProbeFn = fn(ProbeContext) -> Result<ProbeOutcome, ProbeFailure>;

/// 驱动声明。
///
/// Descriptor 只描述驱动如何匹配资源和何时执行 probe，不代表具体设备实例。
#[derive(Clone, Copy)]
pub struct DriverDescriptor {
    /// 驱动稳定诊断名。
    pub name: &'static str,
    /// 驱动支持的资源来源。
    pub probe_kind: ProbeKind,
    /// 匹配后 probe 失败是否阻止启动。
    pub requirement: ProbeRequirement,
    /// 粗粒度 probe 阶段。
    pub level: ProbeLevel,
    /// 同一阶段内的显式优先级。
    pub priority: ProbePriority,
    /// 实际 probe 函数。
    pub probe: ProbeFn,
}

/// probe 资源来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    /// 不依赖 FDT / PCI / ACPI 等枚举资源的静态 probe。
    Static,
    /// 通过 FDT compatible 匹配资源。
    Fdt {
        /// descriptor 声明可匹配的 FDT compatible 字符串。
        compatibles: &'static [&'static str],
    },
}

impl ProbeKind {
    /// 返回 FDT compatible 声明；非 FDT probe 返回空列表。
    pub const fn compatibles(self) -> &'static [&'static str] {
        match self {
            Self::Static => &[],
            Self::Fdt { compatibles } => compatibles,
        }
    }
}

/// 匹配后 probe 失败策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeRequirement {
    /// 匹配后 probe 失败必须 fail-fast。
    Required,
    /// 匹配后 probe 失败只记录诊断并继续。
    Optional,
}

/// probe 阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProbeLevel {
    /// 不依赖总线枚举的基础设施。
    Core,
    /// 提供枚举能力的总线或桥。
    Bus,
    /// 普通设备驱动。
    Device,
    /// 依赖已有 capability 的后置初始化。
    Late,
}

/// 同一 probe level 内的显式优先级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProbePriority(pub i16);

impl ProbePriority {
    /// 默认优先级。
    pub const DEFAULT: Self = Self(0);
}

/// probe 函数输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeContext {
    /// 静态 probe 输入。
    Static,
    /// FDT probe 输入。
    Fdt(FdtProbeContext),
}

/// FDT probe 输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdtProbeContext {
    /// 同一 DTB view 内稳定的节点 id。
    pub node_id: platform_fdt::FdtNodeId,
    /// FDT 节点名。
    pub node_name: platform_fdt::FdtNodeName<'static>,
    /// 命中 descriptor 的 compatible 字符串。
    pub matched_compatible: &'static str,
    /// 当前 D2 首批需要的第一个 `reg` 区域。
    pub reg: platform_fdt::FdtReg,
}

/// probe 结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// descriptor 已经绑定该资源。
    Bound {
        /// 成功注册的设备 id。
        device_id: DeviceId,
    },
    /// descriptor 识别到资源但不绑定它。
    Skipped {
        /// 跳过原因。
        reason: ProbeSkipReason,
    },
}

/// probe 跳过原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeSkipReason {
    /// 资源适用此驱动，但实例类型暂不支持。
    UnsupportedDevice,
    /// 资源不适用于此驱动。
    NotApplicable,
    /// 当前配置禁用该实例。
    Disabled,
}

/// probe 失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeFailure {
    /// 失败分类。
    pub kind: ProbeFailureKind,
    /// 静态诊断说明。
    pub detail: &'static str,
}

impl ProbeFailure {
    /// 构造 probe 失败。
    pub const fn new(kind: ProbeFailureKind, detail: &'static str) -> Self {
        Self { kind, detail }
    }
}

/// probe 失败分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeFailureKind {
    /// 资源描述非法。
    InvalidResource,
    /// transport 初始化失败。
    TransportInitFailed,
    /// 设备初始化失败。
    DeviceInitFailed,
    /// 设备 I/O 失败。
    IoFailed,
    /// 资源或设备类型不支持。
    Unsupported,
}
