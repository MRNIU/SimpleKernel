/**
 * @file kernel_fdt.hpp
 * @brief 用于解析内核自身的 fdt 解析
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-07-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-07-15<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_FDT_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_FDT_HPP_

// 禁用 GCC/Clang 的警告
#include <libfdt_env.h>
#ifdef __GNUC__
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wpedantic"
#endif

#include <libfdt.h>

#ifdef __GNUC__
#pragma GCC diagnostic pop
#endif

#include <array>
#include <cstddef>
#include <cstdint>
#include <tuple>
#include <utility>

#include "kernel_log.hpp"
#include "singleton.hpp"

/**
 * fdt 相关
 */
class KernelFdt {
 public:
  fdt_header *fdt_header_;

  /**
   * 构造函数
   * @param fdt_addr fdt 地址
   */
  explicit KernelFdt(uint64_t header)
      : fdt_header_(reinterpret_cast<fdt_header *>(header)) {
    if (fdt_header_ == nullptr) {
      ERR("Fatal Error: Invalid fdt_addr.\n");
      throw;
    }

    // 检查 fdt 头数据
    if (fdt_check_header(fdt_header_) != 0) {
      ERR("Invalid device tree blob [0x%p]\n", fdt_header_);
      DEBUG("fdt_header_->magic 0x%X\n", fdt_header_->magic);
      DEBUG_BLOB(fdt_header_, 32);
      throw;
    }

    DEBUG("Load dtb at [0x%X], size [0x%X]\n", fdt_header_,
          fdt_header_->totalsize);
  }

  /// @name 构造/析构函数
  /// @{
  KernelFdt() = default;
  KernelFdt(const KernelFdt &) = default;
  KernelFdt(KernelFdt &&) = default;
  auto operator=(const KernelFdt &) -> KernelFdt & = default;
  auto operator=(KernelFdt &&) -> KernelFdt & = default;
  ~KernelFdt() = default;
  /// @}

  /**
   * 获取 core 数量
   * @return core 数量
   */
  [[nodiscard]] auto GetCoreCount() const -> size_t {
    size_t core_count = 0;
    auto offset = -1;

    while (true) {
      offset = fdt_next_node(fdt_header_, offset, nullptr);
      if (offset < 0) {
        break;
      }

      const auto *prop =
          fdt_get_property(fdt_header_, offset, "device_type", nullptr);
      if (prop != nullptr) {
        const char *device_type = reinterpret_cast<const char *>(prop->data);
        if (strcmp(device_type, "cpu") == 0) {
          ++core_count;
        }
      }
    }

    return core_count;
  }

  /**
   * 判断 psci 信息
   */
  void CheckPSCI() const {
    // Find the PSCI node
    auto offset = fdt_path_offset(fdt_header_, "/psci");
    if (offset < 0) {
      ERR("Error finding /psci node: %s\n", fdt_strerror(offset));
      return;
    }

    // Get the method property
    int len = 0;
    const auto *method_prop =
        fdt_get_property(fdt_header_, offset, "method", &len);
    if (method_prop == nullptr) {
      ERR("Error finding PSCI method property\n");
      return;
    }

    // Determine the method (SMC or HVC)
    const char *method_str = reinterpret_cast<const char *>(method_prop->data);
    DEBUG("PSCI method: %s\n", method_str);

    // 暂时只支持 smc
    if (strcmp(method_str, "smc") != 0) {
      ERR("Unsupported PSCI method: %s\n", method_str);
    }

    // Log function IDs for debugging
    auto assert_function_id = [&](const char *name, uint64_t value) {
      const auto *prop = fdt_get_property(fdt_header_, offset, name, &len);
      if (prop != nullptr && (size_t)len >= sizeof(uint32_t)) {
        uint32_t id =
            fdt32_to_cpu(*reinterpret_cast<const uint32_t *>(prop->data));
        DEBUG("PSCI %s function ID: 0x%X\n", name, id);
        if (id != value) {
          ERR("PSCI %s function ID mismatch: expected 0x%X, got 0x%X\n", name,
              value, id);
        }
      }
    };

    /// @see https://developer.arm.com/documentation/den0022/fb/?lang=en
    assert_function_id("cpu_on", 0xC4000003);
    assert_function_id("cpu_off", 0x84000002);
    assert_function_id("cpu_suspend", 0xC4000001);
  }

  /**
   * 获取内存信息
   * @return 内存信息<地址，长度>
   */
  [[nodiscard]] auto GetMemory() const -> std::pair<uint64_t, size_t> {
    uint64_t base = 0;
    uint64_t size = 0;

    int len = 0;

    // 找到 /memory 节点
    auto offset = fdt_path_offset(fdt_header_, "/memory");
    if (offset < 0) {
      ERR("Error finding /memory node: %s\n", fdt_strerror(offset));
      throw;
    }

    // 获取 reg 属性
    const auto *prop = fdt_get_property(fdt_header_, offset, "reg", &len);
    if (prop == nullptr) {
      ERR("Error finding reg property: %s\n", fdt_strerror(len));
      throw;
    }

    // 解析 reg 属性，通常包含基地址和大小
    const auto *reg = reinterpret_cast<const uint64_t *>(prop->data);
    for (size_t i = 0; i < len / sizeof(uint64_t); i += 2) {
      base = fdt64_to_cpu(reg[i]);
      size = fdt64_to_cpu(reg[i + 1]);
    }
    return {base, size};
  }

  /**
   * 获取串口信息
   * @return 内存信息<地址，长度>
   */
  [[nodiscard]] auto GetSerial() const -> std::pair<uint64_t, size_t> {
    uint64_t base = 0;
    uint64_t size = 0;
    int len = 0;

    // Find the /chosen node
    int chosen_offset = fdt_path_offset(fdt_header_, "/chosen");
    if (chosen_offset < 0) {
      ERR("Error finding /chosen node: %s\n", fdt_strerror(chosen_offset));
      throw;
    }

    // Get the stdout-path property
    const auto *prop =
        fdt_get_property(fdt_header_, chosen_offset, "stdout-path", &len);
    if (prop == nullptr || len <= 0) {
      ERR("Error finding stdout-path property: %s\n", fdt_strerror(len));
      throw;
    }

    // Get the path as a string
    const char *stdout_path = reinterpret_cast<const char *>(prop->data);

    // Create a copy of the path that we can modify
    std::array<char, 256> path_buffer;
    strncpy(path_buffer.data(), stdout_path, path_buffer.max_size());

    // Extract the path without any parameters (everything before ':')
    char *colon = strchr(path_buffer.data(), ':');
    if (colon != nullptr) {
      *colon = '\0';  // Terminate the string at the colon
    }

    // Find the node at the stdout path
    int stdout_offset = -1;

    // Handle aliases (paths starting with '&')
    if (path_buffer[0] == '&') {
      const char *alias = path_buffer.data() + 1;  // Skip the '&'
      const char *aliased_path = fdt_get_alias(fdt_header_, alias);
      if (aliased_path != nullptr) {
        stdout_offset = fdt_path_offset(fdt_header_, aliased_path);
      }
    } else {
      stdout_offset = fdt_path_offset(fdt_header_, path_buffer.data());
    }

    if (stdout_offset < 0) {
      ERR("Error finding node for stdout-path %s: %s\n", path_buffer,
          fdt_strerror(stdout_offset));
      throw;
    }

    // Get the reg property of the stdout device
    prop = fdt_get_property(fdt_header_, stdout_offset, "reg", &len);
    if (prop == nullptr) {
      ERR("Error finding reg property for stdout device: %s\n",
          fdt_strerror(len));
      throw;
    }

    // Parse the reg property to get base address and size
    const auto *reg = reinterpret_cast<const uint64_t *>(prop->data);
    for (size_t i = 0; i < len / sizeof(uint64_t); i += 2) {
      base = fdt64_to_cpu(reg[i]);
      size = fdt64_to_cpu(reg[i + 1]);
    }

    return {base, size};
  }

  /**
   * 获取 cpu 时钟
   * @return uint32_t 时钟频率
   */
  [[nodiscard]] auto GetTimebaseFrequency() const -> uint32_t {
    int len = 0;

    // 找到 /cpus 节点
    auto offset = fdt_path_offset(fdt_header_, "/cpus");
    if (offset < 0) {
      ERR("Error finding /cpus node: %s\n", fdt_strerror(offset));
      return 0;
    }

    const auto *prop = reinterpret_cast<const uint32_t *>(
        fdt_getprop(fdt_header_, offset, "timebase-frequency", &len));
    if (prop == nullptr) {
      ERR("Error finding timebase-frequency property: %s\n", fdt_strerror(len));
      return 0;
    }

    if (len != sizeof(uint32_t)) {
      ERR("Unexpected timebase-frequency size\n");
      return 0;
    }

    return fdt32_to_cpu(*prop);
  }

  /**
   * 获取 gic 信息
   * @return 内存信息<dist 地址，redist 地址>
   * @note 仅支持单个 dist+redist
   * @see https://github.com/qemu/qemu/blob/master/hw/arm/virt.c
   */
  [[nodiscard]] auto GetGic() const -> std::pair<uint64_t, uint64_t> {
    uint64_t dist_base = 0;
    uint64_t redist_base = 0;

    int len = 0;
    int offset = 0;

    std::array<const char *, 1> compatible_str = {"arm,gic-v3"};

    for (const auto &compatible : compatible_str) {
      offset = fdt_node_offset_by_compatible(fdt_header_, -1, compatible);
      if (offset != -FDT_ERR_NOTFOUND) {
        break;
      }
    }
    if (offset < 0) {
      ERR("Error finding interrupt controller node: %s\n",
          fdt_strerror(offset));
      throw;
    }

    // 获取 reg 属性
    const auto *prop = fdt_get_property(fdt_header_, offset, "reg", &len);
    if (prop == nullptr) {
      ERR("Error finding reg property: %s\n", fdt_strerror(len));
      throw;
    }

    // 解析 reg 属性，通常包含基地址和大小
    const auto *reg = reinterpret_cast<const uint64_t *>(prop->data);
    if (static_cast<unsigned>(len) >= 2 * sizeof(uint64_t)) {
      dist_base = fdt64_to_cpu(reg[0]);
    }
    if (static_cast<unsigned>(len) >= 4 * sizeof(uint64_t)) {
      redist_base = fdt64_to_cpu(reg[2]);
    }

    return {dist_base, redist_base};
  }

  /**
   * 获取 aarch64 中断号
   * @return intid
   * @see
   * https://www.kernel.org/doc/Documentation/devicetree/bindings/arm/arch_timer.txt
   * https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/Documentation/devicetree/bindings/arm/gic.txt?id=refs/tags/v3.16-rc4
   */
  [[nodiscard]] auto GetAarch64Intid(const char *compatible) const -> uint64_t {
    int len = 0;
    int offset = 0;

    offset = fdt_node_offset_by_compatible(fdt_header_, -1, compatible);

    if (offset < 0) {
      ERR("Error finding interrupt controller node: %s\n",
          fdt_strerror(offset));
      throw;
    }

    // 获取 interrupts 属性
    const auto *prop =
        fdt_get_property(fdt_header_, offset, "interrupts", &len);
    if (prop == nullptr) {
      ERR("Error finding interrupts property: %s\n", fdt_strerror(len));
      throw;
    }

    const auto *interrupts = reinterpret_cast<const uint32_t *>(prop->data);

#ifdef SIMPLEKERNEL_DEBUG
    for (uint32_t i = 0; i < fdt32_to_cpu(prop->len);
         i += 3 * sizeof(uint32_t)) {
      // 0: SPI, 1: PPI
      auto type = fdt32_to_cpu(interrupts[i / sizeof(uint32_t) + 0]);
      // PPI: 16 + intid, SPI: 32 + intid
      // SPI[0-987], PPI[0-15]
      auto intid = fdt32_to_cpu(interrupts[i / sizeof(uint32_t) + 1]);
      auto trigger = fdt32_to_cpu(interrupts[i / sizeof(uint32_t) + 2]) & 0xF;
      auto cpuid_mask =
          fdt32_to_cpu(interrupts[i / sizeof(uint32_t) + 2]) & 0xFF00;
      DEBUG("type: %d, intid: %d, trigger: %d, cpuid_mask: %d\n", type, intid,
            trigger, cpuid_mask);
    }
#endif

    uint64_t intid = 0;
    if (strcmp(compatible, "arm,armv8-timer") == 0) {
      intid = fdt32_to_cpu(interrupts[7]);
    } else if (strcmp(compatible, "arm,pl011") == 0) {
      intid = fdt32_to_cpu(interrupts[1]);
    }

    return intid;
  }

  /**
   * 获取 plic 信息
   * @return 内存信息<地址，长度，中断源数量，上下文数量>
   * @see https://github.com/qemu/qemu/blob/master/hw/arm/virt.c
   */

  [[nodiscard]] auto GetPlic() const
      -> std::tuple<uint64_t, uint64_t, uint32_t, uint32_t> {
    uint64_t base_addr = 0;
    uint64_t size = 0;
    uint32_t ndev = 0;
    uint32_t context_count = 0;

    int len = 0;
    int offset = 0;

    std::array<const char *, 2> compatible_str = {"sifive,plic-1.0.0",
                                                  "riscv,plic0"};

    for (const auto &compatible : compatible_str) {
      offset = fdt_node_offset_by_compatible(fdt_header_, -1, compatible);
      if (offset != -FDT_ERR_NOTFOUND) {
        break;
      }
    }
    if (offset < 0) {
      ERR("Error finding interrupt controller node: %s\n",
          fdt_strerror(offset));
      throw;
    }

    // 通过 interrupts-extended 字段计算上下文数量
    auto prop =
        fdt_get_property(fdt_header_, offset, "interrupts-extended", &len);
    if (prop == nullptr) {
      throw;
    }

    // interrupts-extended 格式: <cpu_phandle interrupt_id> 成对出现
    // 每两个 uint32_t 值表示一个上下文 (CPU + 模式)
    uint32_t num_entries = len / sizeof(uint32_t);
    // 每两个条目表示一个上下文
    context_count = num_entries / 2;

    // 获取 ndev 属性
    prop = fdt_get_property(fdt_header_, offset, "riscv,ndev", &len);
    if (prop == nullptr) {
      throw;
    }
    ndev = fdt32_to_cpu(*reinterpret_cast<const uint32_t *>(prop->data));

    // 获取 reg 属性
    prop = fdt_get_property(fdt_header_, offset, "reg", &len);
    if (prop == nullptr) {
      throw;
    }

    const auto *reg = reinterpret_cast<const uint64_t *>(prop->data);
    base_addr = fdt64_to_cpu(reg[0]);
    size = fdt64_to_cpu(reg[1]);

    return {base_addr, size, ndev, context_count};
  }
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_KERNEL_FDT_HPP_ */
