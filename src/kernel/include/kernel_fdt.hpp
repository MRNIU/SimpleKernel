
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
      klog::Err("Invalid device tree blob [0x%p]\n", fdt_header_);
      klog::Debug("fdt_header_->magic 0x%X\n", fdt_header_->magic);
      klog::DebugBlob(fdt_header_, 32);
      throw;
    }

    klog::Debug("Load dtb at [0x%X], size [0x%X]\n", fdt_header_,
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
   * 获取 psci 信息
   * @return psci 信息
   * @todo 等待 uboot patch 合入
   */
  [[nodiscard]] auto GetPSCI() const -> size_t {
    size_t method = 0;

    // Find the PSCI node
    auto offset = fdt_path_offset(fdt_header_, "/psci");
    if (offset < 0) {
      klog::Err("Error finding /psci node: %s\n", fdt_strerror(offset));
      return 0;
    }

    // Get the method property
    int len = 0;
    const auto *method_prop =
        fdt_get_property(fdt_header_, offset, "method", &len);
    if (method_prop == nullptr) {
      klog::Err("Error finding PSCI method property\n");
      return 0;
    }

    // Determine the method (SMC or HVC)
    const char *method_str = reinterpret_cast<const char *>(method_prop->data);
    klog::Debug("PSCI method: %s\n", method_str);

    if (strcmp(method_str, "smc") == 0) {
      method = 1;  // SMC method
    } else if (strcmp(method_str, "hvc") == 0) {
      method = 2;  // HVC method
    }

    // Log function IDs for debugging
    auto log_function_id = [&](const char *name) {
      const auto *prop = fdt_get_property(fdt_header_, offset, name, &len);
      if (prop != nullptr && (size_t)len >= sizeof(uint32_t)) {
        uint32_t id =
            fdt32_to_cpu(*reinterpret_cast<const uint32_t *>(prop->data));
        klog::Debug("PSCI %s function ID: 0x%X\n", name, id);
      }
    };

    log_function_id("cpu_on");
    log_function_id("cpu_off");
    log_function_id("cpu_suspend");
    log_function_id("system_off");
    log_function_id("system_reset");

    return method;
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

    // Find the /chosen node
    int chosen_offset = fdt_path_offset(fdt_header_, "/chosen");
    if (chosen_offset < 0) {
      klog::Err("Error finding /chosen node: %s\n",
                fdt_strerror(chosen_offset));
      throw;
    }

    // Get the stdout-path property
    const auto *prop =
        fdt_get_property(fdt_header_, chosen_offset, "stdout-path", &len);
    if (prop == nullptr || len <= 0) {
      klog::Err("Error finding stdout-path property: %s\n", fdt_strerror(len));
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
      klog::Err("Error finding node for stdout-path %s: %s\n", path_buffer,
                fdt_strerror(stdout_offset));
      throw;
    }

    // Get the reg property of the stdout device
    prop = fdt_get_property(fdt_header_, stdout_offset, "reg", &len);
    if (prop == nullptr) {
      klog::Err("Error finding reg property for stdout device: %s\n",
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
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_FDT_HPP_ */
