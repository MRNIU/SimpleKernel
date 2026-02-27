/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kernel_elf.hpp"

#include <gtest/gtest.h>

#include "../../src/libcxx/kstd_iostream.cpp"
#include "kernel.elf.h"

TEST(KernelElfTest, DefaultConstructorTest) {
  KernelElf kerlen_elf;
  EXPECT_EQ(kerlen_elf.strtab_, nullptr);
  EXPECT_EQ(kerlen_elf.symtab_.size(), 0);
}

TEST(KernelElfTest, ConstructorWithElfAddrTest) {
  KernelElf kerlen_elf((uint64_t)kernel_elf_data);
  EXPECT_EQ((uint64_t)kerlen_elf.strtab_,
            (uint64_t)((uint64_t)&kernel_elf_data + 0x38d08));
  EXPECT_EQ(kerlen_elf.symtab_.size(), 341);
}

TEST(KernelElfTest, ConstructorWithElfAddrAndElfSizeTest) {
  KernelElf kerlen_elf((uint64_t)kernel_elf_data);
  EXPECT_EQ((uint64_t)kerlen_elf.strtab_,
            (uint64_t)((uint64_t)&kernel_elf_data + 0x38d08));
  EXPECT_EQ(kerlen_elf.symtab_.size(), 341);
}

TEST(KernelElfTest, DefaultAssignConstructorTest) {
  KernelElf kerlen_elf;
  KernelElf kerlen_elf2((uint64_t)kernel_elf_data);
  EXPECT_EQ(kerlen_elf.strtab_, nullptr);
  EXPECT_EQ(kerlen_elf.symtab_.size(), 0);

  kerlen_elf = kerlen_elf2;

  EXPECT_EQ((uint64_t)kerlen_elf.strtab_,
            (uint64_t)((uint64_t)&kernel_elf_data + 0x38d08));
  EXPECT_EQ(kerlen_elf.symtab_.size(), 341);
}

TEST(KernelElfTest, DefaultCopyConstructorTest) {
  KernelElf kerlen_elf((uint64_t)kernel_elf_data);

  KernelElf kerlen_elf2(kerlen_elf);

  EXPECT_EQ((uint64_t)kerlen_elf2.strtab_,
            (uint64_t)((uint64_t)&kernel_elf_data + 0x38d08));
  EXPECT_EQ(kerlen_elf2.symtab_.size(), 341);
}

TEST(KernelElfTest, GetElfSizeTest) {
  KernelElf kerlen_elf((uint64_t)kernel_elf_data);
  EXPECT_GT(kerlen_elf.GetElfSize(), 0);
}

TEST(KernelElfTest, SymtabAccessTest) {
  KernelElf kerlen_elf((uint64_t)kernel_elf_data);

  // 测试符号表是否可以访问
  EXPECT_GT(kerlen_elf.symtab_.size(), 0);

  // 测试符号表内容
  bool found_symbol = false;
  for (const auto& sym : kerlen_elf.symtab_) {
    if (sym.st_name != 0 && kerlen_elf.strtab_ != nullptr) {
      const char* sym_name =
          reinterpret_cast<const char*>(kerlen_elf.strtab_ + sym.st_name);
      if (sym_name != nullptr && strlen(sym_name) > 0) {
        found_symbol = true;
        break;
      }
    }
  }
  EXPECT_TRUE(found_symbol);
}

TEST(KernelElfTest, StrtabNotNullTest) {
  KernelElf kerlen_elf((uint64_t)kernel_elf_data);
  EXPECT_NE(kerlen_elf.strtab_, nullptr);
}

TEST(KernelElfTest, MoveConstructorTest) {
  KernelElf kerlen_elf((uint64_t)kernel_elf_data);
  auto original_strtab = kerlen_elf.strtab_;
  auto original_symtab_size = kerlen_elf.symtab_.size();

  KernelElf kerlen_elf2(std::move(kerlen_elf));

  EXPECT_EQ(kerlen_elf2.strtab_, original_strtab);
  EXPECT_EQ(kerlen_elf2.symtab_.size(), original_symtab_size);
}

TEST(KernelElfTest, MoveAssignmentTest) {
  KernelElf kerlen_elf((uint64_t)kernel_elf_data);
  auto original_strtab = kerlen_elf.strtab_;
  auto original_symtab_size = kerlen_elf.symtab_.size();

  KernelElf kerlen_elf2;
  kerlen_elf2 = std::move(kerlen_elf);

  EXPECT_EQ(kerlen_elf2.strtab_, original_strtab);
  EXPECT_EQ(kerlen_elf2.symtab_.size(), original_symtab_size);
}
