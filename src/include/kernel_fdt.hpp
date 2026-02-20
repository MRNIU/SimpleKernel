/**
 * @copyright Copyright The SimpleKernel Contributors
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

#include <cstddef>
#include <cstdint>
#include <tuple>
#include <utility>

#include "expected.hpp"
#include "kernel_log.hpp"
#include "singleton.hpp"
#include "sk_cassert"

/**
 * @brief FDT（Flattened Device Tree）解析器
 *
 * 提供对 FDT 的解析功能，包括节点查找、属性读取、设备枚举等。
 *
 * @pre  FDT 数据必须是有效的 DTB 格式
 * @post 通过各 Get* 方法可获取设备树中的硬件信息
 *
 * @note ForEachNode 和 ForEachCompatibleNode 是模板方法，保留在头文件中
 * @note compatible 属性是 stringlist 格式（多个以 '\0' 分隔的字符串），
 *       回调接收完整的 compatible 数据和长度
 */
class KernelFdt {
 public:
  /**
   * @brief 构造函数
   * @param header fdt 数据地址
   * @pre header 指向有效的 DTB 数据
   * @post fdt_header_ 已初始化并通过校验
   */
  explicit KernelFdt(uint64_t header);

  /// @name 构造/析构函数
  /// @{
  KernelFdt() = default;
  KernelFdt(const KernelFdt&) = default;
  KernelFdt(KernelFdt&&) = default;
  auto operator=(const KernelFdt&) -> KernelFdt& = default;
  auto operator=(KernelFdt&&) -> KernelFdt& = default;
  ~KernelFdt() = default;
  /// @}

  /**
   * @brief 获取 CPU 核心数量
   * @return Expected<size_t> 成功返回核心数，失败返回错误
   * @pre fdt_header_ 不为空
   */
  [[nodiscard]] auto GetCoreCount() const -> Expected<size_t>;

  /**
   * @brief 判断 PSCI 信息
   * @return Expected<void> 成功返回空，失败返回错误
   * @pre fdt_header_ 不为空
   */
  [[nodiscard]] auto CheckPSCI() const -> Expected<void>;

  /**
   * @brief 获取内存信息
   * @return Expected<std::pair<uint64_t, size_t>> 内存信息<地址，长度>
   * @pre fdt_header_ 不为空
   * @post 返回第一个 reg 条目的 base 和 size
   */
  [[nodiscard]] auto GetMemory() const -> Expected<std::pair<uint64_t, size_t>>;

  /**
   * @brief 获取串口信息
   * @return Expected<std::tuple<uint64_t, size_t, uint32_t>>
   *         <地址，长度，中断号>
   * @pre fdt_header_ 不为空
   */
  [[nodiscard]] auto GetSerial() const
      -> Expected<std::tuple<uint64_t, size_t, uint32_t>>;

  /**
   * @brief 获取 CPU 时钟频率
   * @return Expected<uint32_t> 时钟频率
   * @pre fdt_header_ 不为空
   */
  [[nodiscard]] auto GetTimebaseFrequency() const -> Expected<uint32_t>;

  /**
   * @brief 获取 GIC 信息
   * @return Expected<std::tuple<...>>
   *         <dist地址，dist大小，redist地址，redist大小>
   * @pre fdt_header_ 不为空
   */
  [[nodiscard]] auto GetGIC() const
      -> Expected<std::tuple<uint64_t, size_t, uint64_t, size_t>>;

  /**
   * @brief 获取 GIC Distributor 信息
   * @return Expected<std::pair<uint64_t, size_t>>
   */
  [[nodiscard]] auto GetGicDist() const
      -> Expected<std::pair<uint64_t, size_t>>;

  /**
   * @brief 获取 GIC CPU Interface (Redistributor) 信息
   * @return Expected<std::pair<uint64_t, size_t>>
   */
  [[nodiscard]] auto GetGicCpu() const -> Expected<std::pair<uint64_t, size_t>>;

  /**
   * @brief 获取 aarch64 中断号
   * @param compatible 要查找的 compatible 字符串
   * @return Expected<uint64_t> intid
   * @pre fdt_header_ 不为空
   */
  [[nodiscard]] auto GetAarch64Intid(const char* compatible) const
      -> Expected<uint64_t>;

  /**
   * @brief 获取 PLIC 信息
   * @return Expected<std::tuple<...>>
   *         <地址，长度，中断源数量，上下文数量>
   * @pre fdt_header_ 不为空
   * @see https://github.com/qemu/qemu/blob/master/hw/arm/virt.c
   */
  [[nodiscard]] auto GetPlic() const
      -> Expected<std::tuple<uint64_t, uint64_t, uint32_t, uint32_t>>;

  /**
   * @brief 遍历 FDT 中所有设备节点
   * @tparam Callback 回调类型，签名：
   *   bool(const char* node_name, const char* compatible_data,
   *        size_t compatible_len, uint64_t mmio_base, size_t mmio_size,
   *        uint32_t irq)
   *   返回 true 继续遍历，false 停止
   * @param callback 节点处理函数
   * @return Expected<void>
   * @pre fdt_header_ 不为空
   * @note compatible_data 是完整的 stringlist（多个字符串以 '\0' 分隔），
   *       compatible_len 是整个 stringlist 的字节长度。
   *       若需要显示第一个 compatible 字符串，直接使用 compatible_data 即可。
   *       若需要遍历所有 compatible 字符串，需按 '\0' 分隔迭代。
   */
  template <typename Callback>
  [[nodiscard]] auto ForEachNode(Callback&& callback) const -> Expected<void> {
    sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

    int offset = -1;
    int depth = 0;

    while (true) {
      offset = fdt_next_node(fdt_header_, offset, &depth);
      if (offset < 0) {
        if (offset == -FDT_ERR_NOTFOUND) {
          break;
        }
        return std::unexpected(Error(ErrorCode::kFdtParseFailed));
      }

      const char* node_name = fdt_get_name(fdt_header_, offset, nullptr);
      if (node_name == nullptr) {
        continue;
      }

      int status_len = 0;
      const auto* status_prop =
          fdt_get_property(fdt_header_, offset, "status", &status_len);
      if (status_prop != nullptr) {
        const char* status = reinterpret_cast<const char*>(status_prop->data);
        if (strcmp(status, "okay") != 0 && strcmp(status, "ok") != 0) {
          continue;
        }
      }

      const char* compatible_data = nullptr;
      size_t compatible_len = 0;
      int compat_len = 0;
      const auto* compat_prop =
          fdt_get_property(fdt_header_, offset, "compatible", &compat_len);
      if (compat_prop != nullptr && compat_len > 0) {
        compatible_data = reinterpret_cast<const char*>(compat_prop->data);
        compatible_len = static_cast<size_t>(compat_len);
      }

      uint64_t mmio_base = 0;
      size_t mmio_size = 0;
      int reg_len = 0;
      const auto* reg_prop =
          fdt_get_property(fdt_header_, offset, "reg", &reg_len);
      if (reg_prop != nullptr &&
          static_cast<size_t>(reg_len) >= 2 * sizeof(uint64_t)) {
        const auto* reg = reinterpret_cast<const uint64_t*>(reg_prop->data);
        mmio_base = fdt64_to_cpu(reg[0]);
        mmio_size = fdt64_to_cpu(reg[1]);
      }

      uint32_t irq = 0;
      int irq_len = 0;
      const auto* irq_prop =
          fdt_get_property(fdt_header_, offset, "interrupts", &irq_len);
      if (irq_prop != nullptr &&
          static_cast<size_t>(irq_len) >= sizeof(uint32_t)) {
        const auto* interrupts =
            reinterpret_cast<const uint32_t*>(irq_prop->data);
        irq = fdt32_to_cpu(interrupts[0]);
      }

      if (!callback(node_name, compatible_data, compatible_len, mmio_base,
                    mmio_size, irq)) {
        break;
      }
    }

    return {};
  }

  /**
   * @brief 遍历所有匹配指定 compatible 的 FDT 节点
   * @tparam Callback 回调类型，签名：
   *   bool(int offset, const char* node_name,
   *        uint64_t mmio_base, size_t mmio_size, uint32_t irq)
   *   返回 true 继续遍历，false 停止
   * @param compatible 要匹配的 compatible 字符串
   * @param callback 节点处理函数
   * @return Expected<void>
   * @pre fdt_header_ 不为空
   * @note 使用 fdt_node_offset_by_compatible 的迭代模式，
   *       可正确处理多个节点共享相同 compatible 的情况
   */
  template <typename Callback>
  [[nodiscard]] auto ForEachCompatibleNode(const char* compatible,
                                           Callback&& callback) const
      -> Expected<void> {
    sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

    int offset = -1;
    while (true) {
      offset = fdt_node_offset_by_compatible(fdt_header_, offset, compatible);
      if (offset < 0) {
        if (offset == -FDT_ERR_NOTFOUND) {
          break;
        }
        return std::unexpected(Error(ErrorCode::kFdtParseFailed));
      }

      const char* node_name = fdt_get_name(fdt_header_, offset, nullptr);
      if (node_name == nullptr) {
        continue;
      }

      uint64_t mmio_base = 0;
      size_t mmio_size = 0;
      int reg_len = 0;
      const auto* reg_prop =
          fdt_get_property(fdt_header_, offset, "reg", &reg_len);
      if (reg_prop != nullptr &&
          static_cast<size_t>(reg_len) >= 2 * sizeof(uint64_t)) {
        const auto* reg = reinterpret_cast<const uint64_t*>(reg_prop->data);
        mmio_base = fdt64_to_cpu(reg[0]);
        mmio_size = fdt64_to_cpu(reg[1]);
      }

      uint32_t irq = 0;
      int irq_len = 0;
      const auto* irq_prop =
          fdt_get_property(fdt_header_, offset, "interrupts", &irq_len);
      if (irq_prop != nullptr &&
          static_cast<size_t>(irq_len) >= sizeof(uint32_t)) {
        const auto* interrupts =
            reinterpret_cast<const uint32_t*>(irq_prop->data);
        irq = fdt32_to_cpu(interrupts[0]);
      }

      if (!callback(offset, node_name, mmio_base, mmio_size, irq)) {
        break;
      }
    }

    return {};
  }

 private:
  /// FDT header 指针
  fdt_header* fdt_header_{nullptr};

  /// PSCI 标准函数 ID（SMC64 调用约定）
  /// @see https://developer.arm.com/documentation/den0022/fb/?lang=en
  /// @note 高位 0xC4 表示 SMC64 快速调用，0x84 表示 SMC32 快速调用
  static constexpr uint64_t kPsciCpuOnFuncId = 0xC4000003;
  static constexpr uint64_t kPsciCpuOffFuncId = 0x84000002;
  static constexpr uint64_t kPsciCpuSuspendFuncId = 0xC4000001;

  /**
   * @brief 验证 FDT header
   * @return Expected<void>
   */
  [[nodiscard]] auto ValidateFdtHeader() const -> Expected<void>;

  /**
   * @brief 按路径查找节点
   * @param path 节点路径（如 "/memory"）
   * @return Expected<int> 节点偏移量
   */
  [[nodiscard]] auto FindNode(const char* path) const -> Expected<int>;

  /**
   * @brief 按 compatible 查找第一个匹配的节点
   * @param compatible 要查找的 compatible 字符串
   * @return Expected<int> 节点偏移量
   */
  [[nodiscard]] auto FindCompatibleNode(const char* compatible) const
      -> Expected<int>;

  /**
   * @brief 根据 compatible 查找已启用的节点（跳过 status="disabled"）
   * @param compatible 要查找的 compatible 字符串
   * @return Expected<int> 节点偏移量
   */
  [[nodiscard]] auto FindEnabledCompatibleNode(const char* compatible) const
      -> Expected<int>;

  /**
   * @brief 获取 reg 属性（返回第一个 reg 条目）
   * @param offset 节点偏移量
   * @return Expected<std::pair<uint64_t, size_t>> base 和 size
   * @post 返回第一个 <base, size> 对，而非最后一个
   */
  [[nodiscard]] auto GetRegProperty(int offset) const
      -> Expected<std::pair<uint64_t, size_t>>;

  /**
   * @brief 按 device_type 统计节点数量
   * @param device_type 设备类型
   * @return Expected<size_t> 节点数量
   */
  [[nodiscard]] auto CountNodesByDeviceType(const char* device_type) const
      -> Expected<size_t>;

  /**
   * @brief 获取 PSCI method 属性
   * @param offset 节点偏移
   * @return Expected<const char*> method 字符串
   */
  [[nodiscard]] auto GetPsciMethod(int offset) const -> Expected<const char*>;

  /**
   * @brief 验证 PSCI 函数 ID
   * @param offset 节点偏移
   * @return Expected<void> 验证结果
   */
  [[nodiscard]] auto ValidatePsciFunctionIds(int offset) const
      -> Expected<void>;
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_KERNEL_FDT_HPP_ */
