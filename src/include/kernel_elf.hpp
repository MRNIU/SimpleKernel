/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_ELF_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_ELF_HPP_

#include <elf.h>

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <span>

#include "kernel_log.hpp"
#include "singleton.hpp"

/**
 * elf 文件相关
 */
class KernelElf {
 public:
  /// 符号表
  std::span<Elf64_Sym> symtab_;
  /// 字符串表
  uint8_t* strtab_ = nullptr;

  /**
   * 构造函数
   * @param elf_addr elf 地址
   */
  explicit KernelElf(uint64_t elf_addr) {
    if (elf_addr == 0U) {
      klog::Err("Fatal Error: Invalid elf_addr[0x%lX].\n", elf_addr);
      while (true) {
        cpu_io::Pause();
      }
    }

    elf_ = std::span<uint8_t>(reinterpret_cast<uint8_t*>(elf_addr), EI_NIDENT);

    // 检查 elf 头数据
    auto check_elf_identity_ret = CheckElfIdentity();
    if (!check_elf_identity_ret) {
      klog::Err("KernelElf NOT valid ELF file.\n");
      while (true) {
        cpu_io::Pause();
      }
    }

    ehdr_ = *reinterpret_cast<const Elf64_Ehdr*>(elf_.data());

    // 重新计算 elf 大小
    size_t max_size = EI_NIDENT;
    if (ehdr_.e_phoff != 0) {
      size_t ph_end = ehdr_.e_phoff + ehdr_.e_phnum * ehdr_.e_phentsize;
      if (ph_end > max_size) {
        max_size = ph_end;
      }
    }
    if (ehdr_.e_shoff != 0) {
      size_t sh_end = ehdr_.e_shoff + ehdr_.e_shnum * ehdr_.e_shentsize;
      if (sh_end > max_size) {
        max_size = sh_end;
      }
      const auto* shdrs =
          reinterpret_cast<const Elf64_Shdr*>(elf_.data() + ehdr_.e_shoff);
      for (int i = 0; i < ehdr_.e_shnum; ++i) {
        size_t section_end = shdrs[i].sh_offset + shdrs[i].sh_size;
        if (section_end > max_size) {
          max_size = section_end;
        }
      }
    }
    elf_ = std::span<uint8_t>(reinterpret_cast<uint8_t*>(elf_addr), max_size);

    phdr_ = std::span<Elf64_Phdr>(
        reinterpret_cast<Elf64_Phdr*>(elf_.data() + ehdr_.e_phoff),
        ehdr_.e_phnum);

    shdr_ = std::span<Elf64_Shdr>(
        reinterpret_cast<Elf64_Shdr*>(elf_.data() + ehdr_.e_shoff),
        ehdr_.e_shnum);

    const auto* shstrtab = reinterpret_cast<const char*>(elf_.data()) +
                           shdr_[ehdr_.e_shstrndx].sh_offset;
    for (auto shdr : shdr_) {
      klog::Debug("sh_name: [%s]\n", shstrtab + shdr.sh_name);
      if (strcmp(shstrtab + shdr.sh_name, ".symtab") == 0) {
        symtab_ = std::span<Elf64_Sym>(
            reinterpret_cast<Elf64_Sym*>(elf_.data() + shdr.sh_offset),
            (shdr.sh_size / sizeof(Elf64_Sym)));
      } else if (strcmp(shstrtab + shdr.sh_name, ".strtab") == 0) {
        strtab_ = elf_.data() + shdr.sh_offset;
      }
    }
  }

  /// @name 构造/析构函数
  /// @{
  KernelElf() = default;
  KernelElf(const KernelElf&) = default;
  KernelElf(KernelElf&&) = default;
  auto operator=(const KernelElf&) -> KernelElf& = default;
  auto operator=(KernelElf&&) -> KernelElf& = default;
  ~KernelElf() = default;
  /// @}

  /**
   * 获取 elf 文件大小
   * @return elf 文件大小
   */
  [[nodiscard]] auto GetElfSize() const -> size_t { return elf_.size(); }

 protected:
  /// @name elf 文件相关
  /// @{
  std::span<uint8_t> elf_;
  Elf64_Ehdr ehdr_ = {};
  std::span<Elf64_Phdr> phdr_;
  std::span<Elf64_Shdr> shdr_;
  /// @}

  /**
   * 检查 elf 标识
   * @return 失败返回 false
   */
  [[nodiscard]] auto CheckElfIdentity() const -> bool {
    if ((elf_[EI_MAG0] != ELFMAG0) || (elf_[EI_MAG1] != ELFMAG1) ||
        (elf_[EI_MAG2] != ELFMAG2) || (elf_[EI_MAG3] != ELFMAG3)) {
      klog::Err("Fatal Error: Invalid ELF header.\n");
      while (true) {
        cpu_io::Pause();
      }
    }
    if (elf_[EI_CLASS] == ELFCLASS32) {
      klog::Err("Found 32bit executable but NOT SUPPORT.\n");
      while (true) {
        cpu_io::Pause();
      }
    }
    if (elf_[EI_CLASS] != ELFCLASS64) {
      klog::Err("Fatal Error: Invalid executable.\n");
      while (true) {
        cpu_io::Pause();
      }
    }
    return true;
  }
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_KERNEL_ELF_HPP_ */
