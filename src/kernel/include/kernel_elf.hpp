
/**
 * @file kernel_elf.hpp
 * @brief 用于解析内核自身的 elf
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

#ifndef SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_ELF_HPP_
#define SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_ELF_HPP_

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
  uint8_t *strtab_ = nullptr;

  /**
   * 构造函数
   * @param elf_addr elf 地址
   * @param elf_size elf 大小，默认为 64，Elf64_Ehdr 的大小
   */
  explicit KernelElf(uint64_t elf_addr, size_t elf_size = 64) {
    if ((elf_addr == 0U) || (elf_size == 0U)) {
      klog::Err("Fatal Error: Invalid elf_addr or elf_size.\n");
      throw;
    }

    elf_ = std::span<uint8_t>(reinterpret_cast<uint8_t *>(elf_addr), elf_size);

    // 检查 elf 头数据
    auto check_elf_identity_ret = CheckElfIdentity();
    if (!check_elf_identity_ret) {
      klog::Err("KernelElf NOT valid ELF file.\n");
      throw;
    }

    ehdr_ = *reinterpret_cast<const Elf64_Ehdr *>(elf_.data());

    phdr_ = std::span<Elf64_Phdr>(
        reinterpret_cast<Elf64_Phdr *>(elf_.data() + ehdr_.e_phoff),
        ehdr_.e_phnum);

    shdr_ = std::span<Elf64_Shdr>(
        reinterpret_cast<Elf64_Shdr *>(elf_.data() + ehdr_.e_shoff),
        ehdr_.e_shnum);

    const auto *shstrtab = reinterpret_cast<const char *>(elf_.data()) +
                           shdr_[ehdr_.e_shstrndx].sh_offset;
    for (auto shdr : shdr_) {
      klog::Debug("sh_name: [%s]\n", shstrtab + shdr.sh_name);
      if (strcmp(shstrtab + shdr.sh_name, ".symtab") == 0) {
        symtab_ = std::span<Elf64_Sym>(
            reinterpret_cast<Elf64_Sym *>(elf_.data() + shdr.sh_offset),
            (shdr.sh_size / sizeof(Elf64_Sym)));
      } else if (strcmp(shstrtab + shdr.sh_name, ".strtab") == 0) {
        strtab_ = elf_.data() + shdr.sh_offset;
      }
    }
  }

  /// @name 构造/析构函数
  /// @{
  KernelElf() = default;
  KernelElf(const KernelElf &) = default;
  KernelElf(KernelElf &&) = default;
  auto operator=(const KernelElf &) -> KernelElf & = default;
  auto operator=(KernelElf &&) -> KernelElf & = default;
  ~KernelElf() = default;
  /// @}

 private:
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
      throw;
    }
    if (elf_[EI_CLASS] == ELFCLASS32) {
      klog::Err("Found 32bit executable but NOT SUPPORT.\n");
      throw;
    }
    if (elf_[EI_CLASS] != ELFCLASS64) {
      klog::Err("Fatal Error: Invalid executable.\n");
      throw;
    }
    return true;
  }
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_ELF_HPP_ */
