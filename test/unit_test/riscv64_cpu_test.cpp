
/**
 * @file riscv64_cpu_test.cpp
 * @brief riscv64 cpu 相关测试
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-09-02
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-09-02<td>Zone.N<td>创建文件
 * </table>
 */

#include <gtest/gtest.h>

#include "riscv64/include/cpu/cpu.hpp"

TEST(Riscv64RegInfoBaseTest, ValueTest) {
  EXPECT_EQ(cpu::register_info::RegInfoBase::kBitOffset, 0);
  EXPECT_EQ(cpu::register_info::RegInfoBase::kBitWidth, 64);
  EXPECT_EQ(cpu::register_info::RegInfoBase::kBitMask, 0xFFFFFFFFFFFFFFFF);
  EXPECT_EQ(cpu::register_info::RegInfoBase::kAllSetMask, 0xFFFFFFFFFFFFFFFF);
}

TEST(Riscv64FpInfoTest, ValueTest) {
  EXPECT_EQ(cpu::register_info::FpInfo::kBitOffset, 0);
  EXPECT_EQ(cpu::register_info::FpInfo::kBitWidth, 64);
  EXPECT_EQ(cpu::register_info::FpInfo::kBitMask, 0xFFFFFFFFFFFFFFFF);
  EXPECT_EQ(cpu::register_info::FpInfo::kAllSetMask, 0xFFFFFFFFFFFFFFFF);
}
