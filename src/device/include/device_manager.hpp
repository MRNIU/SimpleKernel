/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <span>

#include "kernel_log.hpp"

struct EnvironmentTraits {
  static auto Log(const char* fmt, ...) -> int {
    klog::Info("%s\n", fmt);
    return 0;
  }
  static auto Mb() -> void { cpu_io::Mb(); }
  static auto Rmb() -> void { cpu_io::Rmb(); }
  static auto Wmb() -> void { cpu_io::Wmb(); }
  static auto VirtToPhys(void* p) -> uint64_t {
    return reinterpret_cast<uint64_t>(p);
  }
  static auto PhysToVirt(uint64_t a) -> void* {
    return reinterpret_cast<void*>(a);
  }
};

struct MmioTransport {
  uint64_t base_addr;
};

struct PciTransport {
  uint64_t bus;
  uint64_t device;
  uint64_t function;
};

/**
 * @brief 设备接口基类（非模板），用于类型擦除
 *
 * 所有设备无论使用何种 Transport，都通过此接口统一管理。
 * DeviceManager 通过此基类指针数组持有所有设备，实现对不同
 * Transport 设备的统一存储和操作。
 */
struct DeviceBase {
  enum class DeviceType { Blk, Network };

  virtual ~DeviceBase() = default;
  virtual void HandleInterrupt() = 0;
  virtual int Read() = 0;
  virtual int Write() = 0;

  size_t GetInterruptId() const { return interrupt_id_; }

  size_t interrupt_id_{0};
  DeviceType type_;
};

/**
 * @brief 带有具体传输层的设备基类
 *
 * 继承自非模板的 DeviceBase，将 Transport 信息保存在派生层，
 * 从而实现类型擦除：DeviceManager 只需持有 DeviceBase* 即可。
 *
 * @tparam Transport 设备访问的传输层类型，如 MmioTransport、PciTransport
 */
template <class Transport>
struct TransportDevice : public DeviceBase {
  explicit TransportDevice(Transport transport) : transport_(transport) {}
  Transport transport_;
};

/**
 * @brief 块设备类
 */
template <class Transport>
struct BlkDevice : public TransportDevice<Transport> {
  using TransportDevice<Transport>::TransportDevice;
};

/**
 * @brief 网络设备类
 */
template <class Transport>
struct NetworkDevice : public TransportDevice<Transport> {
  using TransportDevice<Transport>::TransportDevice;
};

/**
 * @brief 设备管理器，负责管理系统中的所有设备
 * 通过 compatible
 * 字符串识别设备类型，创建对应的设备对象，并提供统一的接口进行设备操作。
 *
 * 通过非模板基类 DeviceBase 实现类型擦除，使得不同 Transport
 * 类型的设备可以统一存储在同一个数组中。
 *
 * @tparam EnvironmentTraits
 * 平台环境特征类型，提供日志、内存屏障、地址转换等功能
 * @tparam MaxDevices 设备管理器支持的最大设备数量
 */
template <class EnvironmentTraits, size_t MaxDevices = 16>
class DeviceManager {
 public:
  // 通过 compatible 字符串来识别设备类型，创建对应的设备对象
  // 通过 transport 传递设备相关的参数（如 MMIO 基地址，或 pci
  // 设备的信息），通过 irq_num 传递设备的中断号
  template <class Transport>
  DeviceBase* AddDevice(const char* compatible_str, Transport transport) {
    // TODO: 根据 compatible_str 创建具体设备并加入 devices_
    return nullptr;
  }

  void HandleInterrupt(size_t source_id) {
    for (size_t i = 0; i < MaxDevices; ++i) {
      if (devices_[i].valid &&
          devices_[i].device->GetInterruptId() == source_id) {
        devices_[i].device->HandleInterrupt();
        break;
      }
    }
  }

  DeviceManager() {}

 private:
  struct DeviceInfo {
    DeviceBase* device;
    const char* compatible_str;
    bool valid;
  };

  std::array<DeviceInfo, MaxDevices> devices_{};
};
