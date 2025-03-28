
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

#ifndef SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_FDT_HPP_
#define SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_FDT_HPP_

// 禁用 GCC/Clang 的警告
#include "libfdt_env.h"
#ifdef __GNUC__
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wpedantic"
#endif

#include <libfdt.h>

#ifdef __GNUC__
#pragma GCC diagnostic pop
#endif

#include <cstddef>
#include <cstdint>
#include <utility>

#include "kernel_log.hpp"
#include "singleton.hpp"

/**
 * fdt 相关
 */
class KernelFdt {
 public:
  struct FdtHeader {
    uint32_t magic;
    uint32_t totalsize;
    uint32_t off_dt_struct;
    uint32_t off_dt_strings;
    uint32_t off_mem_rsvmap;
    uint32_t version;
    uint32_t last_comp_version;
    uint32_t boot_cpuid_phys;
    uint32_t size_dt_strings;
    uint32_t size_dt_struct;
  };

  FdtHeader *fdt_header_;

  /**
   * 构造函数
   * @param fdt_addr fdt 地址
   */
  explicit KernelFdt(uint64_t header)
      : fdt_header_(reinterpret_cast<FdtHeader *>(header)) {
    if (fdt_header_ == nullptr) {
      klog::Err("Fatal Error: Invalid fdt_addr.\n");
      throw;
    }

    // 检查 fdt 头数据
    if (fdt_check_header(fdt_header_) != 0) {
      klog::Err("Invalid device tree blob\n");
      throw;
    }
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
    int offset = 0;
    while (true) {
      offset = fdt_node_offset_by_compatible(fdt_header_, offset, "riscv");
      if (offset < 0) {
        break;
      }
      ++core_count;
    }
    return core_count;
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
      klog::Err("Error finding /memory node: %s\n", fdt_strerror(offset));
      throw;
    }

    // 获取 reg 属性
    const auto *prop = fdt_get_property(fdt_header_, offset, "reg", &len);
    if (prop == nullptr) {
      klog::Err("Error finding reg property: %s\n", fdt_strerror(len));
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
    int offset = 0;

    std::array<const char *, 2> compatible_str = {"arm,pl011\0arm,primecell",
                                                  "ns16550a"};

    for (const auto &compatible : compatible_str) {
      offset = fdt_node_offset_by_compatible(fdt_header_, -1, compatible);
      if (offset != -FDT_ERR_NOTFOUND) {
        break;
      }
    }
    if (offset < 0) {
      klog::Err("Error finding /soc/serial node: %s\n", fdt_strerror(offset));
      throw;
    }

    // 获取 reg 属性
    const auto *prop = fdt_get_property(fdt_header_, offset, "reg", &len);
    if (prop == nullptr) {
      klog::Err("Error finding reg property: %s\n", fdt_strerror(len));
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
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_FDT_HPP_ */
