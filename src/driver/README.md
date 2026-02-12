

struct EnvironmentTraits {
  static auto Log(const char* fmt, ...) -> int {
    klog::Info("%s\n", fmt);
    return 0;
  }
  static auto Mb() -> void { asm volatile("fence iorw, iorw" ::: "memory"); }
  static auto Rmb() -> void { asm volatile("fence ir, ir" ::: "memory"); }
  static auto Wmb() -> void { asm volatile("fence ow, ow" ::: "memory"); }
  static auto VirtToPhys(void* p) -> uintptr_t {
    return reinterpret_cast<uintptr_t>(p);
  }
  static auto PhysToVirt(uintptr_t a) -> void* {
    return reinterpret_cast<void*>(a);
  }
};

// 最上层，统一接口
template <class EnvironmentTraits, size_t MaxDevices = 16,
template <class> class TransportT>
class DeviceManager{
public:

  // 中间层，设备抽象
  struct DeviceBase {
    DeviceBase(size_t interrupt_id) : interrupt_id_(interrupt_id) {};

    virtual void HandleInterrupt() = 0;
    virtual int Read() = 0;
    virtual int Write() = 0;

    size_t GetInterruptId() const { return interrupt_id_; }

    size_t interrupt_id_;
    DeviceType type_;
  };

  struct BlkDevice : public DeviceBase {
    // 最下层，具体驱动
    BlkDevice() {
      // 初始化驱动
    }

  };
  struct NetworkDevice : public DeviceBase {};

  // 通过 compiatble 字符串来识别设备类型，创建对应的设备对象
  // 通过 transport 传递设备相关的参数（如 MMIO 基地址，或 pci 设备的信息），通过 irq_num 传递设备的中断号
  DeviceBase* AddDevice(const char * compiatble_str, TransportT transport, size_t irq_num){}
  AddBlkDevice();
  DeviceManager();

  HandleInterrupt(size_t source_id){
    for (size_t i = 0; i < MaxDevices; ++i) {
      if (devices_[i] && devices_[i]->GetInterruptId() == source_id) {
        // 调用设备的中断处理函数
        devices_[i]->HandleInterrupt();
        break;
      }
    }
  }

private:
  std::array<BlkDevice*, MaxDevices> devices_;
};

int main(int,char**){
  // 初始化设备管理器
  auto device_manager = device_manager::DeviceManager<EnvironmentTraits>::GetInstance();
  // 初始化设备
  auto dev = device_manager.AddDevice("virtio,mmio", mmio_base, irq_num);
  if (dev->type_ == DeviceType::Blk) {
    klog::Info("Blk device added\n");
  } else if (dev->type_ == DeviceType::Network) {
    klog::Info("Network device added\n");
  } else {
    klog::Err("Unknown device type\n");
  }

  // 注册外部中断控制器的中断处理函数
  RegisterExternalInterruptControllerHandler(dev.get_interrupt_id(), [](uint64_t source_id, uint8_t*) -> uint64_t {
    device_manager::DeviceManager<EnvironmentTraits>::GetInstance().HandleInterrupt(source_id);
      return 0;
    });

  dev.Read();
  dev.Write();

  return 0;
}
