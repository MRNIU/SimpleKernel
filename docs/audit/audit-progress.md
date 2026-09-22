<!-- Copyright The SimpleKernel Contributors -->

# 审计进度

> 更新：2026-09-22。事实基线：`feat/rust-SAS`，`f5e1f5f96de1d52af29adf0c2b3b7cb8cf0e2b53`。
> 第二轮开始时 index / 工作区干净，后续环境与依赖刷新保留此前未提交整理。本文件只维护当前交接；旧摘要与验证记录见
> [审计历史记录](2026-09-22-audit-history.md)，交付物完成状态只在 [Roadmap](review-roadmap.md) 维护。

## 当前状态

项目仍处于 R0–R8 深度审计中的阶段性收口。最近实现主线是 R6 设备框架：
D0.5、D1、D2 及指定兼容入口清理、跨架构 FDT 流式枚举已落地；
当前为第二轮开发协作入口收敛及作者追加的环境/依赖刷新，D3 仍在 readiness / 范围讨论。
**D2 完成不等于 R6 完成，也不等于全项目审计完成。**

- 实现边界只在 [设备子系统当前设计](../design/device-subsystem-current.md) 维护；
  `646f5638` 统一容量常量，`e45053ab` 删除旧 D2 文档，第一轮已将有效契约并入现状文档。
- R3/R4 已有多轮修复和历史验证；R5/R7 未找到完整审计交付与关闭证据。
  各阶段欠项、文档漂移和关闭条件见 Roadmap，不从旧“已完成目标”表推导阶段完成。
- [style follow-up](2026-06-10-style-organization-followup.md) 是已完成切片的历史追踪，不再作为待执行清单。

## 当前工作与交接

第一轮对账成果已核验，交接及旧静态统计归入
[第一轮记录](2026-09-22-audit-history.md#collaboration-round-one)。第二轮收敛 README、
贡献指南、根/局部 AGENTS、环境与 xtask 手册、PR 模板、审计 prompt 和 VS Code 适配，
新增唯一 [SimpleKernel 开发 skill](../../.agents/skills/simplekernel-dev/SKILL.md)。
详细规则各归所属入口，不再在根 AGENTS / README 维护完整副本。

作者追加范围已实施：Docker/Dev Container 改为可选，删除 `.editorconfig` 与 issue 模板，
参考 eha_controller 简化中文提交模板、增加 TOML 检查和直接 Cargo hooks；刷新 nightly、
Cargo、Actions 与镜像工具。仅适配 `arm-gic` 构造 API，补齐 clap 帮助 feature，未扩展内核能力、
重写 xtask/测试框架或改变 ADR 状态。DMA 保持兼容线，固件 gitlink 保持原版本。
[依赖核验](dependency-audit.md) 记录取舍与统计。Git diff 是待 review 交付物；未提交或 push。

## 下一个目标

**唯一优先任务：作者 review 第二轮协作入口、skill 及环境/依赖刷新的工作区 diff。**
本轮已创建 skill，不再沿用“下一轮创建”的旧指令；review 后再由作者确定后续开发范围。

设备开发线的恢复入口仍是 **D3 readiness 范围确认**，不是直接实现 D3。
root/default block 选择、分区块设备、`Late` 后置初始化、多能力查询只是候选，
最小目标、失败语义和验收证据尚待作者确认，没有确定 D3a/D3b 顺序，也没有接受历史推荐方案。

## 未决事项与触发条件

- D3：选择哪个最小目标、是否改变首个 Block 的默认选择、失败/回退语义、需要哪些测试，均待确认。
- R6：VFS、FD、FAT 及 POSIX 兼容范围仍需交付物与审计证据；FAT 成功挂载不能由 `fs-test` 的成功直接推断。
- R3/R4：运行期权限切换需要并发写者证明；多 bank RAM、真机 DMA 边界见
  [设备与 DMA 跟踪](2026-05-07-device-dma-rdrive-tracking.md)。TLB mailbox/rendezvous、
  missed tick/tickless 仅按 ADR-018/019 的触发条件回看，不作为本轮新增必做项。
- R7/R8：syscall 唯一公共网关仍是目标；当前 `src/lib.rs` 还公开多个子系统。
  全局可见性、测试基础设施和阶段关闭证据见 Roadmap。历史建议不自动转为 ADR 决策。

## 验证证据入口

| 证据 | 可说明什么 | 不能说明什么 |
|------|------------|--------------|
| [D2c / 清理记录](2026-09-22-audit-history.md#d2c-validation) | 当时记载的 host、双架构检查、RISC-V 定点及全量回归 | 本轮 HEAD 已执行验证、R6 关闭 |
| [跨架构 FDT 枚举记录](2026-09-22-audit-history.md#d2-cross-arch-validation) | 2026-06-10 记载双架构各 32 个独立测试通过 | 当前 HEAD 全绿、FAT 必然挂载成功 |
| [TLB 远端访问记录](2026-09-22-audit-history.md#tlb-validation) | 2026-06-08 记载双架构 ack 后旧权限失效测试 | R4 全阶段关闭、未来运行期并发模型已证明 |
| [style 验证记录](2026-09-22-audit-history.md#style-validation) | 当时风格切片的扫描、检查和定点测试 | 全项目安全审计或 R8 关闭 |

历史记录未完整保存每次测试时 HEAD 和原始日志，保留其证据层级，不补造绑定。
本轮验证结果与工具限制见下节；不复用第一轮的静态统计作为本轮证据。

## 第二轮验证与工具限制（2026-09-22）

早期纯文档核验见 [依赖刷新前的快照](2026-09-22-audit-history.md#collaboration-round-two-initial)。
以下为提交 `6ff91fa4` 所收敛的验证记录，不表示审计阶段关闭或真实硬件已验证。
该提交之后的基础镜像精简和 AArch64 对照诊断见文末；旧完整镜像结果不代表精简镜像。

- 在本机已有 devbox 中安装/使用仓库指定 nightly，未安装宿主机开发依赖；这是本机环境选择，
  不是项目对贡献者的容器要求。
- `cargo fmt --all -- --check`、双裸机 target Clippy（`--locked`、`-D warnings`）及
  `cargo clippy --locked -p xtask -- -D warnings` 通过。
- `cargo test --locked -p memory_types -p config -p page_table_entry -p platform_fdt -p device_core -p macros -p xtask`
  共 22 项测试通过、2 项文档示例忽略；补齐 clap feature 后另复跑 xtask 的 4 项测试通过。
- `cargo xtask --help`、`cargo xtask test --help` 与 `cargo xtask test --list` 实际通过，列表 32 项。
  `cargo xtask build --arch riscv64` / `--arch aarch64` 均通过。
- QEMU 10.2.1，双架构各执行一轮 `cargo xtask test --arch <arch> --all --timeout 30`，
  每轮外层预算 600 秒。RISC-V **32 通过 / 0 失败**；AArch64 **1 通过 / 31 失败**，均无超时。
  AArch64 仅 panic-test 通过，其余在固件保留区初始化处失败，不能据此验证升级后的 GIC 运行行为。
  复用当前 `target/firmware/` 产物，未重建/升级固件；结束后确认无残留 QEMU。
  原始日志在仓库内忽略的 `.tmp/dependency-refresh/`。
- `cargo deny check` 通过，保留多版本/path 通配符告警，详情见依赖核验文档。
- 修改文件的 pre-commit、两架构手动 Clippy hook、actionlint 1.7.12、Dockerfile `--check`
  与 skill-creator 格式校验通过；未安装 Git hook。开发镜像完整构建成功，临时本地 tag 为
  `simplekernel-devcontainer:collab-refresh`；以 dev 用户核验 Rust/Node/npm/pre-commit/Mermaid/
  cargo-deny/mdbook 版本及 `/srv/tftp` 写权限通过，未推送镜像。
- 双架构 rustdoc 构建通过；修复此次发现的 `src/fs/fd_table.rs` 中 TaskControlBlock 文档链接。
- `git diff --check` 与受影响 Markdown 本地链接/锚点、xtask 示例和规则引用检查通过。
- skill 仍只有 `.agents/skills/simplekernel-dev/SKILL.md`，发现位置未变；环境选择改为按贡献者意愿。
  再走查局部 bug、只读审查、文档修改：分别选择 package/相关 target 验证、只读证据、链接/diff，
  不强制容器、全仓扫描、重复加载规则或无关 QEMU。

### 限制与工具问题

- **AArch64 系统测试阻塞**：`xtask/src/aarch64_boot_scr.txt:19` 使用
  `bootm $kernel_addr_r - $fdt_addr`，选择 QEMU DTB；`xtask/src/qemu.rs:124` 注入的固件保留区
  位于 FIT 内的另一份 DTB。内核 `src/init.rs:66` 因 `NodeNotFound` fail-fast。
  对生成 DTB 的独立 host 诊断返回 `Ok((0x40000000, 0x100000))`，说明该输入可被新解析器读取；
  串口明确显示使用 `0x40000000` 处的 QEMU FDT。原 HEAD 的启动脚本与初始化要求相同，
  当时未改此启动链，也未重跑旧依赖基线；后续 QEMU 版本对照见文末。
  后续需单独修正 AArch64 DTB 交接并重跑回归；本轮不靠 fallback 放宽内核校验。

- devbox bind mount 上首次 xtask 链接报告临时 `.o` 文件不可见；随后查询文件已存在。
  使用 `CARGO_BUILD_BUILD_DIR=/opt/simplekernel-build-cache` 将中间产物移到容器文件系统后构建成功，
  `CARGO_TARGET_DIR` 仍指向当前 checkout 的 `target/`。这只记录本次环境规避方式，未改构建系统。
- 当前 xtask 仍依赖 Unix symlink、固定 `/srv/tftp` 和 Linux 风格工具名；本地开发可选不等于
  原生 Windows/macOS 全流程已支持。基础镜像精简后，运行 QEMU 前按容器指南准备 `/srv/tftp`。
- VS Code `cppdbg` 扩展需自行安装，后台 debug task 仍缺 readiness matcher；未验证 F5 连通性。
- xtask 的 check / firmware 分发不消费 `ArchArgs.release`；非法测试名仍可能先触发固件准备。
  本轮未改这些行为，手册不提供无效选项，名称不确定时先用 `test --list`。
- 未执行 GitHub 托管 CI、Pages 发布或镜像推送；actionlint 和本机运行不能替代远端结果。

## 基础环境与 AArch64 对照诊断（2026-09-22，基于 6ff91fa4）

- Rust stable 1.98.1：`cargo +stable check --locked -p platform_fdt` 在
  `sync_unsafe_cell` feature gate 报 E0554；`cargo +stable xtask check --arch riscv64`
  在 `-Z build-std` 被拒绝。另有 `alloc_error_handler` 使用，保留仓库 nightly 与 devbox 安装。
- crates 外 AGENTS 从 10 个减至 3 个：根规则及 tests/xtask 的极短导航。
  文档、ADR、模板、测试和 xtask 手册改为 README；RFC/Spec/Plan 规则合并到文档索引，
  容器维护合并到 docker.md。19 个 crates AGENTS 及 crates 下全部文件保持不变。
- 基础镜像仅预装 Git、C/C++ 编译环境和 Rust；CI 系统测试按架构安装固件/QEMU，
  检查 job 单独安装 cargo-deny，Dev Container 仅自动安装 rust-analyzer 扩展。

- 精简镜像 `simplekernel-devcontainer:basic-check` 构建成功；以 dev 用户核验 Rust/Git/C
  可用，QEMU、Node、pre-commit、Mermaid、cargo-deny 未预装。一次性容器内
  `cargo test --locked -p xtask -p platform_fdt` 共 10 项通过；
  `cargo xtask check --arch aarch64` 与 `--arch riscv64` 通过。
  初次验证将 target 放入 Docker 默认 noexec tmpfs，build script 无法执行；改为容器普通目录
  `/opt/check-target` 后通过，未为此修改项目配置。
- actionlint、composite action shell 语法、两架构可选包清单的 `apt-get --simulate`、
  Dockerfile `--check`、修改文件 pre-commit、链接/命令检查和 skill 格式校验通过。
  skill 三场景走查仍分别选择局部测试、只读证据、文档链接检查，无新增全量回归要求。
  未执行托管 CI、从零构建固件或完整系统回归；AArch64 定点诊断不能替代它们。
  日志在 `.tmp/basic-image-check.log`、`.tmp/basic-pre-commit.log`、`.tmp/ci-deps-check.log`。

### AArch64 原因与修复方向

使用同一现有 `fdt-firmware-reserved` ELF、FIT 与固定固件产物，直接执行 xtask 等价 QEMU
参数，每次限制 30 秒。只重打包忽略目录内的 `boot.scr.uimg` 做对照，结束后恢复原产物；
未修改内核或仓库启动脚本，未升级固件。该测试使用 `InitLevel::Memory`，不覆盖完整设备/SMP 启动。

| QEMU | 原脚本：显式使用 QEMU 原始 DTB | 提取 FIT DTB 后显式传入 |
|------|--------------------------------|-------------------------|
| 8.2.2（Ubuntu 24.04） | `src/init.rs:66` / `NodeNotFound` | `TEST OK` |
| 10.2.1（devbox） | `src/init.rs:66` / `NodeNotFound` | `TEST OK` |

因此这次缺少固件保留区的失败不由 QEMU 升级独立引起。`xtask/src/qemu.rs` 把保留区
注入 FIT DTB，但 `xtask/src/aarch64_boot_scr.txt:19` 用原始 `$fdt_addr` 绕过它。
Git 历史：`f25fba40` 引入保留区注入时有缺失节点 fallback；`c0b629c3`（2026-06-10）
删除 fallback 后，这个交接问题变成 fail-fast。不要通过恢复 fallback 隐藏问题。

不能直接改成 `bootm $kernel_addr_r`：虽选中 FIT DTB，实测会在
`src/arch/aarch64/init.rs:21` 报 `argc=1`；当前 ELF 启动约定从 `argv[2]` 解析 DTB。
已验证的修复方向是提取处理后的 DTB，再保留显式参数：

```text
setenv fdt_addr_r 0x43000000
imxtract $kernel_addr_r fdt $fdt_addr_r
bootm $kernel_addr_r - $fdt_addr_r
```

这是诊断方案，不是已经合入的修复。正式实现需检查提取失败时停止、DTB 暂存与内核/FIT
地址区间不重叠，并运行完整 AArch64 回归。当前地址只在上述固定产物对照中验证。
原始串口日志：忽略目录 `.tmp/aarch64-dtb-probe/`、`.tmp/aarch64-dtb-probe-qemu8/`。
两版原脚本虽进程退出码为 0，均含 `TEST PANIC`，按 sentinel 判失败。
