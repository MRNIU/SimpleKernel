/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_DRIVER_GIC_INCLUDE_GIC_H_
#define SIMPLEKERNEL_SRC_KERNEL_DRIVER_GIC_INCLUDE_GIC_H_

#include <array>
#include <cstdint>

#include "io.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"

/**
 * @brief gic 中断控制器驱动
 * @see
 * https://developer.arm.com/documentation/100095/0003/Generic-Interrupt-Controller-CPU-Interface/GIC-programmers-model/CPU-interface-memory-mapped-register-descriptions
 */
class Gic {
 public:
  static constexpr const char* kCompatibleName = "arm,gic-v3";

  static constexpr size_t kSGIBase = 0;
  static constexpr size_t kSGICount = 16;
  static constexpr size_t kPPIBase = 16;
  static constexpr size_t kPPICount = 16;
  static constexpr size_t kSPIBase = 32;
  static constexpr size_t kSPICount = 988;

  class Gicd {
   public:
    /// @see
    /// https://developer.arm.com/documentation/101206/0003/Programmers-model/Distributor-registers--GICD-GICDA--summary
    /// GICD Register offsets
    /// Configuration dependent Distributor Control Register, RW
    static constexpr uint32_t kCTLR = 0x0000;
    static constexpr uint32_t kCTLR_EnableGrp1NS = 0x2;
    /**
     * @brief GICD_CTLR, Distributor Control Register
     * @see
     * https://developer.arm.com/documentation/101206/0003/Programmers-model/Distributor-registers--GICD-GICDA--summary/GICD-CTLR--Distributor-Control-Register?lang=en
     */
    struct GICD_CTLR {
      // [31] Register Write Pending:
      uint32_t rwp : 1;
      uint32_t reserved1 : 23;
      // [7] E1NWF Enable 1 of N Wakeup Functionality
      uint32_t e1nwf : 1;
      // [6] DS Disable Security
      uint32_t ds : 1;
      // [5] ARE_NS Affinity Routing Enable, Non-secure state
      uint32_t are_ns : 1;
      // [4] ARE_S Affinity Routing Enable, Secure state
      uint32_t are_s : 1;
      uint32_t reserved0 : 1;
      // [2] EnableGrp1S Enable Secure Group 1 interrupts
      uint32_t enable_grp1_s : 1;
      // [1] EnableGrp1NS Enable Non-secure Group 1 interrupts
      uint32_t enable_grp1_ns : 1;
      // [0] EnableGrp0 Enable Group 0 interrupts
      uint32_t enable_grp0 : 1;
    };

    /// Configuration dependent Interrupt Controller Type Register, RO
    static constexpr uint32_t kTYPER = 0x0004;
    static constexpr uint32_t kTYPER_ITLinesNumberMask = 0x1F;
    /**
     * @brief GICD_TYPER, Interrupt Controller Type Register
     * @see
     * https://developer.arm.com/documentation/101206/0003/Programmers-model/Distributor-registers--GICD-GICDA--summary/GICD-TYPER--Interrupt-Controller-Type-Register?lang=en
     */
    struct GICD_TYPER {
      // [31:26] Reserved, returns 0b000000
      uint32_t reserved1 : 6;
      // [25] No1N 1 of N SPI
      uint32_t no1n : 1;
      // [24] A3V Affinity level 3 values.
      uint32_t a3v : 1;
      // [23:19] IDbits Interrupt identifier bits
      uint32_t idbits : 5;
      // [18] DVIS Direct virtual LPI injection support
      uint32_t dvis : 1;
      // [17] LPIS Indicates whether the implementation supports LPIs.
      uint32_t lpis : 1;
      // [16] MBIS Message-based interrupt support
      uint32_t mbis : 1;
      // [15:11] num_LPIs Returns 0b00000 because
      // GICD_TYPER.IDbits indicates the number of LPIs that the GIC supports.
      uint32_t num_lpis : 5;
      // [10] SecurityExtn Security state support.
      uint32_t security_extn : 1;
      // [9:8] - Reserved, returns 0b00000
      uint32_t reserved0 : 2;
      // [7:5] CPUNumber Returns 0b000 because GICD_CTLR.ARE==1 (ARE_NS &
      // ARE_S).
      uint32_t cpu_number : 3;
      // [4:0] ITLinesNumber Returns the maximum SPI INTID that this
      // GIC-600AE implementation supports, and is given by 32×(ITLinesNumber +
      // 1) − 1.
      uint32_t it_lines_number : 5;
    };

    /// Configuration dependent Distributor Implementer Identification Register,
    /// RO
    static constexpr uint32_t kIIDR = 0x0008;
    /**
     * @brief GICD_IIDR, Distributor Implementer Identification Register
     * @see
     * https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICD-IIDR--Distributor-Implementer-Identification-Register?lang=en
     */
    struct GICD_IIDR {
      // [31:24] Product Identifier.
      uint32_t product_id : 8;
      // [23:20] Reserved, RES0.
      uint32_t reserved0 : 4;
      // [19:16] Variant number. Typically, this field is used to distinguish
      // product variants, or major revisions of a product.
      uint32_t variant : 4;
      // [15:12] Revision number. Typically, this field is used to distinguish
      // minor revisions of a product.
      uint32_t revision : 4;
      // [11:0] Contains the JEP106 manufacturer's identification code of the
      // designer of the Distributor.
      uint32_t implementer : 12;
    };

    /// Function Control Register, RW
    static constexpr uint32_t kFCTLR = 0x0020;
    /// Tie-off dependentb Secure Access Control register, RW
    static constexpr uint32_t kSAC = 0x0024;
    /// Non-secure SPI Set Register, WO
    static constexpr uint32_t kSETSPI_NSR = 0x0040;
    /// Non-secure SPI Clear Register, WO
    static constexpr uint32_t kCLRSPI_NSR = 0x0048;
    /// Secure SPI Set Register, WO
    static constexpr uint32_t kSETSPI_SR = 0x0050;
    /// Secure SPI Clear Register, WO
    static constexpr uint32_t kCLRSPI_SR = 0x0058;

    /// Interrupt Group Registers, n = 0-31, but n=0 is Reserved
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICD-IGROUPR-n---Interrupt-Group-Registers?lang=en
    static constexpr uint32_t kIGROUPRn = 0x0080;
    __always_inline auto IGROUPRn(uint64_t n) const -> uint64_t {
      return kIGROUPRn + n * 4;
    }

    /// Interrupt Set-Enable Registers, n = 0-31, but n=0 is Reserved
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICD-ISENABLER-n---Interrupt-Set-Enable-Registers?lang=en
    static constexpr uint32_t kISENABLERn = 0x0100;
    static constexpr uint32_t kISENABLERn_SIZE = 32;
    __always_inline auto ISENABLERn(uint64_t n) const -> uint64_t {
      return kISENABLERn + n * 4;
    }

    /// Interrupt Clear-Enable Registers, n = 0-31, but n=0 is Reserved
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICD-ICENABLER-n---Interrupt-Clear-Enable-Registers?lang=en
    static constexpr uint32_t kICENABLERn = 0x0180;
    static constexpr uint32_t kICENABLERn_SIZE = 32;
    __always_inline auto ICENABLERn(uint64_t n) const -> uint64_t {
      return kICENABLERn + n * 4;
    }

    /// Interrupt Set-Pending Registers, n = 0-31, but n=0 is Reserved
    static constexpr uint32_t kISPENDRn = 0x0200;

    /// Interrupt Clear-Pending Registers, n = 0-31, but n=0 is Reserved
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICD-ICPENDR-n---Interrupt-Clear-Pending-Registers?lang=en
    static constexpr uint32_t kICPENDRn = 0x0280;
    static constexpr uint32_t kICPENDRn_SIZE = 32;
    __always_inline auto ICPENDRn(uint64_t n) const -> uint64_t {
      return kICPENDRn + n * 4;
    }

    /// Interrupt Set-Active Registers, n = 0-31, but n=0 is Reserved
    static constexpr uint32_t kISACTIVERn = 0x0300;
    /// Interrupt Clear-Active Registers, n = 0-31, but n=0 is Reserved
    static constexpr uint32_t kICACTIVERn = 0x0380;

    /// Interrupt Priority Registers, n = 0-255, but n=0-7 are Reserved when
    /// affinity routing is enabled
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICD-IPRIORITYR-n---Interrupt-Priority-Registers?lang=en
    static constexpr uint32_t kIPRIORITYRn = 0x0400;
    static constexpr uint32_t kIPRIORITYRn_SIZE = 4;
    static constexpr uint32_t kIPRIORITYRn_BITS = 8;
    static constexpr uint32_t kIPRIORITYRn_BITS_MASK = 0xFF;
    __always_inline auto IPRIORITYRn(uint64_t n) const -> uint64_t {
      return kIPRIORITYRn + n * 4;
    }

    /// Interrupt Processor Targets Registers, n = 0 - 254
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICD-ITARGETSR-n---Interrupt-Processor-Targets-Registers?lang=en
    static constexpr uint32_t kITARGETSRn = 0x0800;
    static constexpr uint32_t kITARGETSRn_SIZE = 4;
    static constexpr uint32_t kITARGETSRn_BITS = 8;
    static constexpr uint32_t kITARGETSRn_BITS_MASK = 0xFF;
    __always_inline auto ITARGETSRn(uint64_t n) const -> uint64_t {
      return kITARGETSRn + n * 4;
    }

    /// Interrupt Configuration Registers, n = 0-63, but n=0-1 are Reserved
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICD-ICFGR-n---Interrupt-Configuration-Registers?lang=en
    static constexpr uint32_t kICFGRn = 0x0C00;
    static constexpr uint32_t kICFGRn_SIZE = 16;
    static constexpr uint32_t kICFGRn_BITS = 2;
    static constexpr uint32_t kICFGRn_BITS_MASK = 0x3;
    static constexpr uint32_t kICFGRn_LevelSensitive = 0;
    static constexpr uint32_t kICFGRn_EdgeTriggered = 1;
    __always_inline auto ICFGRn(uint64_t n) const -> uint64_t {
      return kICFGRn + n * 4;
    }

    /// Interrupt Group Modifier Registers, n = 0-31, but n=0 is Reserved. If
    /// GICD_CTLR.DS == 1, then this register is RAZ/WI.
    static constexpr uint32_t kIGRPMODRn = 0x0D00;
    /// Non-secure Access Control Registers, n = 0-63, but n=0-1 are Reserved
    /// when affinity routing is enabled
    static constexpr uint32_t kNSACRn = 0x0E00;
    /// Interrupt Routing Registers, n = 0-991, but n=0-31 are Reserved when
    /// affinity routing is enabled.
    static constexpr uint32_t kIROUTERn = 0x6000;
    /// P-Channel dependent Chip Status Register, RW
    static constexpr uint32_t kCHIPSR = 0xC000;
    /// Default Chip Register, RW
    static constexpr uint32_t kDCHIPR = 0xC004;
    /// Chip Registers, n = 0-15. Reserved in single-chip configurations.
    static constexpr uint32_t kCHIPRn = 0xC008;
    /// Interrupt Class Registers, n = 0-63, but n=0-1 are Reserved
    static constexpr uint32_t kICLARn = 0xE000;
    /// Interrupt Clear Error Registers, n = 0-31, but n=0 is Reserved
    static constexpr uint32_t kICERRRn = 0xE100;
    /// Configuration dependent Configuration ID Register, RO
    static constexpr uint64_t kCFGID = 0xF000;
    /// Peripheral ID4 register , RO
    static constexpr uint32_t kPIDR4 = 0xFFD0;
    /// Peripheral ID 5 Register, RO
    static constexpr uint32_t kPIDR5 = 0xFFD4;
    /// Peripheral ID 6 Register, RO
    static constexpr uint32_t kPIDR6 = 0xFFD8;
    /// Peripheral ID 7 Register, RO
    static constexpr uint32_t kPIDR7 = 0xFFDC;
    /// Peripheral ID0 register, RO
    static constexpr uint32_t kPIDR0 = 0xFFE0;
    /// Peripheral ID1 register, RO
    static constexpr uint32_t kPIDR1 = 0xFFE4;
    /// Peripheral ID2 register, RO
    static constexpr uint32_t kPIDR2 = 0xFFE8;
    /// Peripheral ID3 register, RO
    static constexpr uint32_t kPIDR3 = 0xFFEC;
    /// Component ID 0 Register, RO
    static constexpr uint32_t kCIDR0 = 0xFFF0;
    /// Component ID 1 Register, RO
    static constexpr uint32_t kCIDR1 = 0xFFF4;
    /// Component ID 2 Register, RO
    static constexpr uint32_t kCIDR2 = 0xFFF8;
    /// Component ID 3 Register, RO
    static constexpr uint32_t kCIDR3 = 0xFFFC;

    /**
     * 构造函数
     * @param dev_addr 设备地址
     */
    explicit Gicd(uint64_t base_addr);

    /// @name 默认构造/析构函数
    /// @{
    Gicd() = default;
    Gicd(const Gicd& na16550a) = delete;
    Gicd(Gicd&& na16550a) = delete;
    auto operator=(const Gicd& na16550a) -> Gicd& = delete;
    auto operator=(Gicd&& na16550a) -> Gicd& = default;
    ~Gicd() = default;
    /// @}

    /**
     * 允许从 Distributor 转发到 redistributor
     * @param intid 中断号
     */
    void Enable(uint32_t intid) const;

    /**
     * 允许 no-sec group1 中断
     */
    void EnableGrp1NS() const {
      Write(kCTLR, kCTLR_EnableGrp1NS);
      cpu_io::ICC_IGRPEN1_EL1::Enable::Set();
    }

    /**
     * 禁止从 Distributor 转发到 redistributor
     * @param intid 中断号
     */
    void Disable(uint32_t intid) const;

    /**
     * 清除 intid 的中断
     * @param intid 中断号
     */
    void Clear(uint32_t intid) const;

    /**
     * 判断 intid 中断是否使能
     * @param intid 中断号
     * @return true 允许
     */
    auto IsEnable(uint32_t intid) const -> bool;

    /**
     * 设置 intid 的优先级
     * @param intid 中断号
     * @param prio 优先级
     */
    void SetPrio(uint32_t intid, uint32_t prio) const;

    /**
     * 设置 intid 的属性
     * @param intid 中断号
     * @param config 属性
     */
    void SetConfig(uint32_t intid, uint32_t config) const;

    /**
     * 设置 intid 的由指定 cpu 处理
     * @param intid 中断号
     * @param cpuid cpu 编号
     */
    void SetTarget(uint32_t intid, uint32_t cpuid) const;

    /**
     * 设置指定 SPI 中断
     * SPI: shared peripheral interrupt,
     * 共享外设中断，该中断来源于外设，但是该中断可以对所有的 core 有效
     * @param intid 中断号
     * @param cpuid cpu 编号
     */
    void SetupSPI(uint32_t intid, uint32_t cpuid) const;

   private:
    uint64_t base_addr_ = 0;

    __always_inline auto Read(uint32_t off) const -> uint32_t {
      return io::In<uint32_t>(base_addr_ + off);
    }

    __always_inline void Write(uint32_t off, uint32_t val) const {
      io::Out<uint32_t>(base_addr_ + off, val);
    }
  };

  class Gicr {
   public:
    /// 每个 GICR 长度 2 * 64 * 1024
    static constexpr uint32_t kSTRIDE = 0x20000;

    /// Redistributor Control Register, RW
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-CTLR--Redistributor-Control-Register?lang=en
    static constexpr uint32_t kCTLR = 0x0000;
    /// Redistributor Implementation Identification Register, RO
    static constexpr uint32_t kIIDR = 0x0004;
    /// Redistributor Type Register, RO
    static constexpr uint32_t kTYPER = 0x0008;

    /// Power Management Control Register, RW1
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-WAKER--Redistributor-Wake-Register?lang=en
    static constexpr uint32_t kWAKER = 0x0014;
    static constexpr uint32_t kWAKER_ProcessorSleepMASK = 2;
    static constexpr uint32_t kWAKER_ChildrenAsleepMASK = 4;

    /// Function Control Register, RW
    static constexpr uint32_t kFCTLR = 0x0020;
    /// Power Register, RW
    static constexpr uint32_t kPWRR = 0x0024;
    /// Class Register, RW
    static constexpr uint32_t kCLASSR = 0x0028;
    /// Redistributor Properties Base Address Register, RW
    static constexpr uint32_t kPROPBASER = 0x0070;
    /// Redistributor LPI Pending Table Base Address Register, RW
    static constexpr uint32_t kPENDBASER = 0x0078;
    /// Peripheral ID 4 Register, RO
    static constexpr uint32_t kPIDR4 = 0xFFD0;
    /// Peripheral ID 5 Register, RO
    static constexpr uint32_t kPIDR5 = 0xFFD4;
    /// Peripheral ID 6 Register, RO
    static constexpr uint32_t kPIDR6 = 0xFFD8;
    /// Peripheral ID 7 Register, RO
    static constexpr uint32_t kPIDR7 = 0xFFDC;
    /// Peripheral ID 0 Register, RO
    static constexpr uint32_t kPIDR0 = 0xFFE0;
    /// Peripheral ID 1 Register, RO
    static constexpr uint32_t kPIDR1 = 0xFFE4;
    /// Peripheral ID 2 Register, RO
    static constexpr uint32_t kPIDR2 = 0xFFE8;
    /// Peripheral ID 3 Register, RO
    static constexpr uint32_t kPIDR3 = 0xFFEC;
    /// Component ID 0 Register, RO
    static constexpr uint32_t kCIDR0 = 0xFFF0;
    /// Component ID 1 Register, RO
    static constexpr uint32_t kCIDR1 = 0xFFF4;
    /// Component ID 2 Register, RO
    static constexpr uint32_t kCIDR2 = 0xFFF8;
    /// Component ID 3 Register, RO
    static constexpr uint32_t kCIDR3 = 0xFFFC;

    /// SGI 基地址 64 * 1024
    static constexpr uint32_t kSGI_BASE = 0x10000;

    /// Interrupt Group Register, RW
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-IGROUPR0--Interrupt-Group-Register-0?lang=en
    static constexpr uint32_t kIGROUPR0 = kSGI_BASE + 0x0080;
    static constexpr uint32_t kIGROUPR0_Clear = 0;
    static constexpr uint32_t kIGROUPR0_Set = UINT32_MAX;

    /// Interrupt Set-Enable Register, RW
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-ISENABLER0--Interrupt-Set-Enable-Register-0?lang=en
    static constexpr uint32_t kISENABLER0 = kSGI_BASE + 0x0100;
    static constexpr uint32_t kISENABLER0_SIZE = 32;

    /// Interrupt Clear-Enable Register, RW
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-ICENABLER0--Interrupt-Clear-Enable-Register-0?lang=en
    static constexpr uint32_t kICENABLER0 = kSGI_BASE + 0x0180;
    static constexpr uint32_t kICENABLER0_SIZE = 32;

    /// Interrupt Set-Pending Register, RW
    static constexpr uint32_t kISPENDR0 = kSGI_BASE + 0x0200;

    /// Peripheral Clear Pending Register, RW
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-ICPENDR0--Interrupt-Clear-Pending-Register-0?lang=en
    static constexpr uint32_t kICPENDR0 = kSGI_BASE + 0x0280;
    static constexpr uint32_t kICPENDR0_SIZE = 32;

    /// Interrupt Set-Active Register, RW
    static constexpr uint32_t kISACTIVER0 = kSGI_BASE + 0x0300;
    /// Interrupt Clear-Active Register, RW
    static constexpr uint32_t kICACTIVER0 = kSGI_BASE + 0x0380;
    /// Interrupt Priority Registers, RW
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-IPRIORITYR-n---Interrupt-Priority-Registers?lang=en
    static constexpr uint32_t kIPRIORITYRn = kSGI_BASE + 0x0400;
    static constexpr uint32_t kIPRIORITYRn_SIZE = 4;
    static constexpr uint32_t kIPRIORITYRn_BITS = 8;
    static constexpr uint32_t kIPRIORITYRn_BITS_MASK = 0xFF;
    __always_inline auto IPRIORITYRn(uint64_t n) const -> uint64_t {
      return kIPRIORITYRn + n * 4;
    }

    /// Interrupt Configuration Registers, RW
    static constexpr uint32_t kICFGRn = 0x0C00;

    /// Interrupt Group Modifier Register, RW
    /// @see
    /// https://developer.arm.com/documentation/ddi0601/2024-12/External-Registers/GICR-IGRPMODR0--Interrupt-Group-Modifier-Register-0?lang=en
    static constexpr uint32_t kIGRPMODR0 = 0x0D00;
    // kIGRPMODR0 kIGROUPR0 Definition
    // 0b0        0b0       Secure Group 0 G0S
    // 0b0        0b1       Non-secure Group 1 G1NS
    // 0b1        0b0       Secure Group 1 G1S
    static constexpr uint32_t kIGRPMODR0_Clear = 0;
    static constexpr uint32_t kIGRPMODR0_Set = UINT32_MAX;

    /// Non-secure Access Control Register, RW
    static constexpr uint32_t kNSACR = 0x0E00;
    /// Miscellaneous Status Register, RO
    static constexpr uint32_t kMISCSTATUSR = 0xC000;
    /// Interrupt Error Valid Register, RW
    static constexpr uint32_t kIERRVR = 0xC008;
    /// SGI Default Register, RW
    static constexpr uint32_t kSGIDR = 0xC010;
    /// Configuration ID0 Register, RO
    static constexpr uint32_t kCFGID0 = 0xF000;
    /// Configuration ID1 Register, RO
    static constexpr uint32_t kCFGID1 = 0xF004;

    /**
     * 构造函数
     * @param dev_addr 设备地址
     */
    explicit Gicr(uint64_t base_addr);

    /// @name 默认构造/析构函数
    /// @{
    Gicr() = default;
    Gicr(const Gicr& na16550a) = delete;
    Gicr(Gicr&& na16550a) = delete;
    auto operator=(const Gicr& na16550a) -> Gicr& = delete;
    auto operator=(Gicr&& na16550a) -> Gicr& = default;
    ~Gicr() = default;
    /// @}

    /**
     * 允许从 redistributor 转发到 CPU interface
     * @param intid 中断号
     * @param cpuid cpu 编号
     */
    void Enable(uint32_t intid, uint32_t cpuid) const;

    /**
     * 禁止从 redistributor 转发到 CPU interface
     * @param intid 中断号
     * @param cpuid cpu 编号
     */
    void Disable(uint32_t intid, uint32_t cpuid) const;

    /**
     * 清除指定 cpu intid 的中断
     * @param intid 中断号
     * @param cpuid cpu 编号
     */
    void Clear(uint32_t intid, uint32_t cpuid) const;

    /**
     * 设置 intid 的优先级
     * @param intid 中断号
     * @param cpuid cpu 编号
     * @param prio 优先级
     */
    void SetPrio(uint32_t intid, uint32_t cpuid, uint32_t prio) const;

    /**
     * 初始化 gicr，在多核场景使用
     */
    void SetUP() const;

    /**
     * 设置指定 PPI 中断
     * PPI: private peripheral interrupt,
     * 私有外设中断，该中断来源于外设，但是该中断只对指定的 core 有效
     * @param intid 中断号
     * @param cpuid cpu 编号
     */
    void SetupPPI(uint32_t intid, uint32_t cpuid) const;

    /**
     * 设置指定 SGI 中断
     * SGI: Software Generated Interrupt,
     * 软件生成中断，用于处理器间通信
     * @param intid 中断号 (0-15)
     * @param cpuid cpu 编号
     */
    void SetupSGI(uint32_t intid, uint32_t cpuid) const;

   private:
    uint64_t base_addr_ = 0;

    __always_inline auto Read(uint32_t cpuid, uint32_t off) const -> uint32_t {
      return io::In<uint32_t>(base_addr_ + cpuid * kSTRIDE + off);
    }

    __always_inline void Write(uint32_t cpuid, uint32_t off,
                               uint32_t val) const {
      io::Out<uint32_t>(base_addr_ + cpuid * kSTRIDE + off, val);
    }
  };

  /**
   * 构造函数
   * @param gicd_base_addr gic distributor 地址
   * @param gicr_base_addr gic redistributor 地址
   */
  explicit Gic(uint64_t gicd_base_addr, uint64_t gicr_base_addr);

  /// @name 默认构造/析构函数
  /// @{
  Gic() = default;
  Gic(const Gic& na16550a) = delete;
  Gic(Gic&& na16550a) = delete;
  auto operator=(const Gic& na16550a) -> Gic& = delete;
  auto operator=(Gic&& na16550a) -> Gic& = default;
  ~Gic() = default;
  /// @}

  void SetUP() const;
  void SPI(uint32_t intid, uint32_t cpuid) const;
  void PPI(uint32_t intid, uint32_t cpuid) const;
  void SGI(uint32_t intid, uint32_t cpuid) const;

 private:
  Gicd gicd_;
  Gicr gicr_;
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_DRIVER_GIC_INCLUDE_GIC_H_ */
