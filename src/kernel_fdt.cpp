/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kernel_fdt.hpp"

#include <array>

KernelFdt::KernelFdt(uint64_t header)
    : fdt_header_(reinterpret_cast<fdt_header*>(header)) {
  ValidateFdtHeader().or_else([](Error err) -> Expected<void> {
    klog::Err("KernelFdt init failed: %s\n", err.message());
    while (true) {
      cpu_io::Pause();
    }
    return {};
  });

  klog::Debug("Load dtb at [0x%X], size [0x%X]\n", fdt_header_,
              fdt32_to_cpu(fdt_header_->totalsize));
}

auto KernelFdt::GetCoreCount() const -> Expected<size_t> {
  sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

  return CountNodesByDeviceType("cpu").and_then(
      [](size_t count) -> Expected<size_t> {
        if (count == 0) {
          return std::unexpected(Error(ErrorCode::kFdtNodeNotFound));
        }
        return count;
      });
}

auto KernelFdt::CheckPSCI() const -> Expected<void> {
  sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

  return FindNode("/psci").and_then([this](int offset) -> Expected<void> {
    return GetPsciMethod(offset).and_then(
        [this, offset](const char* method) -> Expected<void> {
          klog::Debug("PSCI method: %s\n", method);
          if (strcmp(method, "smc") != 0) {
            return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
          }
          return ValidatePsciFunctionIds(offset);
        });
  });
}

auto KernelFdt::GetMemory() const -> Expected<std::pair<uint64_t, size_t>> {
  sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

  return FindNode("/memory").and_then(
      [this](int offset) -> Expected<std::pair<uint64_t, size_t>> {
        return GetRegProperty(offset).transform(
            [](std::pair<uint64_t, size_t> reg) { return reg; });
      });
}

auto KernelFdt::GetSerial() const
    -> Expected<std::tuple<uint64_t, size_t, uint32_t>> {
  sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

  auto chosen_offset = FindNode("/chosen");
  if (!chosen_offset.has_value()) {
    return std::unexpected(chosen_offset.error());
  }

  int len = 0;
  const auto* prop =
      fdt_get_property(fdt_header_, chosen_offset.value(), "stdout-path", &len);
  if (prop == nullptr || len <= 0) {
    return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
  }

  const char* stdout_path = reinterpret_cast<const char*>(prop->data);
  std::array<char, 256> path_buffer;
  strncpy(path_buffer.data(), stdout_path, path_buffer.max_size());

  char* colon = strchr(path_buffer.data(), ':');
  if (colon != nullptr) {
    *colon = '\0';
  }

  int stdout_offset = -1;
  if (path_buffer[0] == '&') {
    const char* alias = path_buffer.data() + 1;
    const char* aliased_path = fdt_get_alias(fdt_header_, alias);
    if (aliased_path != nullptr) {
      stdout_offset = fdt_path_offset(fdt_header_, aliased_path);
    }
  } else {
    stdout_offset = fdt_path_offset(fdt_header_, path_buffer.data());
  }

  if (stdout_offset < 0) {
    return std::unexpected(Error(ErrorCode::kFdtNodeNotFound));
  }

  auto reg = GetRegProperty(stdout_offset);
  if (!reg.has_value()) {
    return std::unexpected(reg.error());
  }

  prop = fdt_get_property(fdt_header_, stdout_offset, "interrupts", &len);
  if (prop == nullptr) {
    return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
  }

  uint32_t irq = 0;
  const auto* interrupts = reinterpret_cast<const uint32_t*>(prop->data);
  if (interrupts != nullptr && len != 0) {
    irq = fdt32_to_cpu(*interrupts);
  }

  return std::tuple{reg.value().first, reg.value().second, irq};
}

auto KernelFdt::GetTimebaseFrequency() const -> Expected<uint32_t> {
  sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

  return FindNode("/cpus").and_then([this](int offset) -> Expected<uint32_t> {
    int len = 0;
    const auto* prop = reinterpret_cast<const uint32_t*>(
        fdt_getprop(fdt_header_, offset, "timebase-frequency", &len));
    if (prop == nullptr) {
      return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
    }

    if (len != sizeof(uint32_t)) {
      return std::unexpected(Error(ErrorCode::kFdtInvalidPropertySize));
    }

    return fdt32_to_cpu(*prop);
  });
}

auto KernelFdt::GetGIC() const
    -> Expected<std::tuple<uint64_t, size_t, uint64_t, size_t>> {
  sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

  auto offset = FindCompatibleNode("arm,gic-v3");
  if (!offset.has_value()) {
    return std::unexpected(offset.error());
  }

  int len = 0;
  const auto* prop = fdt_get_property(fdt_header_, offset.value(), "reg", &len);
  if (prop == nullptr) {
    return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
  }

  uint64_t dist_base = 0;
  size_t dist_size = 0;
  uint64_t redist_base = 0;
  size_t redist_size = 0;

  const auto* reg = reinterpret_cast<const uint64_t*>(prop->data);
  if (static_cast<unsigned>(len) >= 2 * sizeof(uint64_t)) {
    dist_base = fdt64_to_cpu(reg[0]);
    dist_size = fdt64_to_cpu(reg[1]);
  }
  if (static_cast<unsigned>(len) >= 4 * sizeof(uint64_t)) {
    redist_base = fdt64_to_cpu(reg[2]);
    redist_size = fdt64_to_cpu(reg[3]);
  }

  return std::tuple{dist_base, dist_size, redist_base, redist_size};
}

auto KernelFdt::GetGicDist() const -> Expected<std::pair<uint64_t, size_t>> {
  return GetGIC().transform(
      [](std::tuple<uint64_t, size_t, uint64_t, size_t> gic) {
        return std::pair{std::get<0>(gic), std::get<1>(gic)};
      });
}

auto KernelFdt::GetGicCpu() const -> Expected<std::pair<uint64_t, size_t>> {
  return GetGIC().transform(
      [](std::tuple<uint64_t, size_t, uint64_t, size_t> gic) {
        return std::pair{std::get<2>(gic), std::get<3>(gic)};
      });
}

auto KernelFdt::GetAarch64Intid(const char* compatible) const
    -> Expected<uint64_t> {
  sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

  auto offset = FindEnabledCompatibleNode(compatible);
  if (!offset.has_value()) {
    return std::unexpected(offset.error());
  }

  int len = 0;
  const auto* prop =
      fdt_get_property(fdt_header_, offset.value(), "interrupts", &len);
  if (prop == nullptr) {
    return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
  }

  const auto* interrupts = reinterpret_cast<const uint32_t*>(prop->data);

#ifdef SIMPLEKERNEL_DEBUG
  for (uint32_t i = 0; i < fdt32_to_cpu(prop->len); i += 3 * sizeof(uint32_t)) {
    auto type = fdt32_to_cpu(interrupts[i / sizeof(uint32_t) + 0]);
    auto intid = fdt32_to_cpu(interrupts[i / sizeof(uint32_t) + 1]);
    auto trigger = fdt32_to_cpu(interrupts[i / sizeof(uint32_t) + 2]) & 0xF;
    auto cpuid_mask =
        fdt32_to_cpu(interrupts[i / sizeof(uint32_t) + 2]) & 0xFF00;
    klog::Debug("type: %d, intid: %d, trigger: %d, cpuid_mask: %d\n", type,
                intid, trigger, cpuid_mask);
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

auto KernelFdt::GetPlic() const
    -> Expected<std::tuple<uint64_t, uint64_t, uint32_t, uint32_t>> {
  sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");

  auto offset = FindCompatibleNode("sifive,plic-1.0.0");
  if (!offset.has_value()) {
    offset = FindCompatibleNode("riscv,plic0");
  }
  if (!offset.has_value()) {
    return std::unexpected(offset.error());
  }

  int len = 0;

  const auto* prop = fdt_get_property(fdt_header_, offset.value(),
                                      "interrupts-extended", &len);
  if (prop == nullptr) {
    return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
  }

  // interrupts-extended: <cpu_phandle interrupt_id> pairs
  uint32_t num_entries = len / sizeof(uint32_t);
  uint32_t context_count = num_entries / 2;

  prop = fdt_get_property(fdt_header_, offset.value(), "riscv,ndev", &len);
  if (prop == nullptr) {
    return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
  }
  uint32_t ndev = fdt32_to_cpu(*reinterpret_cast<const uint32_t*>(prop->data));

  auto reg = GetRegProperty(offset.value());
  if (!reg.has_value()) {
    return std::unexpected(reg.error());
  }

  return std::tuple{reg.value().first, reg.value().second, ndev, context_count};
}

auto KernelFdt::ValidateFdtHeader() const -> Expected<void> {
  sk_assert_msg(fdt_header_ != nullptr, "fdt_header_ is null");
  if (fdt_check_header(fdt_header_) != 0) {
    return std::unexpected(Error(ErrorCode::kFdtInvalidHeader));
  }
  return {};
}

auto KernelFdt::FindNode(const char* path) const -> Expected<int> {
  auto offset = fdt_path_offset(fdt_header_, path);
  if (offset < 0) {
    return std::unexpected(Error(ErrorCode::kFdtNodeNotFound));
  }
  return offset;
}

auto KernelFdt::FindCompatibleNode(const char* compatible) const
    -> Expected<int> {
  auto offset = fdt_node_offset_by_compatible(fdt_header_, -1, compatible);
  if (offset < 0) {
    return std::unexpected(Error(ErrorCode::kFdtNodeNotFound));
  }
  return offset;
}

auto KernelFdt::FindEnabledCompatibleNode(const char* compatible) const
    -> Expected<int> {
  int offset = -1;
  while (true) {
    offset = fdt_node_offset_by_compatible(fdt_header_, offset, compatible);
    if (offset < 0) {
      return std::unexpected(Error(ErrorCode::kFdtNodeNotFound));
    }

    int len = 0;
    const auto* status_prop =
        fdt_get_property(fdt_header_, offset, "status", &len);

    if (status_prop == nullptr) {
      return offset;
    }

    const char* status = reinterpret_cast<const char*>(status_prop->data);
    if (strcmp(status, "okay") == 0 || strcmp(status, "ok") == 0) {
      return offset;
    }
  }
}

auto KernelFdt::GetRegProperty(int offset) const
    -> Expected<std::pair<uint64_t, size_t>> {
  int len = 0;
  const auto* prop = fdt_get_property(fdt_header_, offset, "reg", &len);
  if (prop == nullptr) {
    return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
  }

  const auto* reg = reinterpret_cast<const uint64_t*>(prop->data);
  if (static_cast<size_t>(len) < 2 * sizeof(uint64_t)) {
    return std::unexpected(Error(ErrorCode::kFdtInvalidPropertySize));
  }

  uint64_t base = fdt64_to_cpu(reg[0]);
  size_t size = fdt64_to_cpu(reg[1]);
  return std::pair{base, size};
}

auto KernelFdt::CountNodesByDeviceType(const char* device_type) const
    -> Expected<size_t> {
  size_t count = 0;
  auto offset = -1;

  while (true) {
    offset = fdt_next_node(fdt_header_, offset, nullptr);
    if (offset < 0) {
      if (offset != -FDT_ERR_NOTFOUND) {
        return std::unexpected(Error(ErrorCode::kFdtParseFailed));
      }
      break;
    }

    const auto* prop =
        fdt_get_property(fdt_header_, offset, "device_type", nullptr);
    if (prop != nullptr) {
      const char* type = reinterpret_cast<const char*>(prop->data);
      if (strcmp(type, device_type) == 0) {
        ++count;
      }
    }
  }

  return count;
}

auto KernelFdt::GetPsciMethod(int offset) const -> Expected<const char*> {
  int len = 0;
  const auto* prop = fdt_get_property(fdt_header_, offset, "method", &len);
  if (prop == nullptr) {
    return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
  }
  return reinterpret_cast<const char*>(prop->data);
}

auto KernelFdt::ValidatePsciFunctionIds(int offset) const -> Expected<void> {
  auto validate_id = [this, offset](const char* name,
                                    uint64_t expected) -> Expected<void> {
    int len = 0;
    const auto* prop = fdt_get_property(fdt_header_, offset, name, &len);
    if (prop != nullptr && static_cast<size_t>(len) >= sizeof(uint32_t)) {
      uint32_t id =
          fdt32_to_cpu(*reinterpret_cast<const uint32_t*>(prop->data));
      klog::Debug("PSCI %s function ID: 0x%X\n", name, id);
      if (id != expected) {
        klog::Err("PSCI %s function ID mismatch: expected 0x%X, got 0x%X\n",
                  name, expected, id);
        return std::unexpected(Error(ErrorCode::kFdtPropertyNotFound));
      }
    }
    return {};
  };

  return validate_id("cpu_on", kPsciCpuOnFuncId)
      .and_then([&]() { return validate_id("cpu_off", kPsciCpuOffFuncId); })
      .and_then(
          [&]() { return validate_id("cpu_suspend", kPsciCpuSuspendFuncId); });
}
