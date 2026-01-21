/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief fdt 解析测试
 */

#include "kernel_fdt.hpp"

#include <gtest/gtest.h>

#include "riscv64_virt.dtb.h"

TEST(KernelFdtTest, ConstructorTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
}

TEST(KernelFdtTest, GetMemoryTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  auto [memory_base, memory_size] = kerlen_fdt.GetMemory();

  EXPECT_EQ(memory_base, 0x80000000);
  EXPECT_EQ(memory_size, 0x8000000);
}

TEST(KernelFdtTest, GetSerialTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  auto [serial_base, serial_size, serial_irq] = kerlen_fdt.GetSerial();

  EXPECT_EQ(serial_base, 0x10000000);
  EXPECT_EQ(serial_size, 0x100);
  EXPECT_EQ(serial_irq, 0xA);
}

TEST(KernelFdtTest, GetTimebaseFrequencyTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  auto time_base_frequency = kerlen_fdt.GetTimebaseFrequency();

  EXPECT_EQ(time_base_frequency, 0x989680);
}

TEST(KernelFdtTest, GetCoreCountTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  auto core_count = kerlen_fdt.GetCoreCount();

  EXPECT_GT(core_count, 0);  // 至少有一个 CPU 核心
}

TEST(KernelFdtTest, CopyConstructorTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
  KernelFdt kerlen_fdt2(kerlen_fdt);

  auto [memory_base1, memory_size1] = kerlen_fdt.GetMemory();
  auto [memory_base2, memory_size2] = kerlen_fdt2.GetMemory();

  EXPECT_EQ(memory_base1, memory_base2);
  EXPECT_EQ(memory_size1, memory_size2);
}

TEST(KernelFdtTest, AssignmentTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
  KernelFdt kerlen_fdt2;

  kerlen_fdt2 = kerlen_fdt;

  auto [memory_base1, memory_size1] = kerlen_fdt.GetMemory();
  auto [memory_base2, memory_size2] = kerlen_fdt2.GetMemory();

  EXPECT_EQ(memory_base1, memory_base2);
  EXPECT_EQ(memory_size1, memory_size2);
}

TEST(KernelFdtTest, MoveConstructorTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
  auto [expected_base, expected_size] = kerlen_fdt.GetMemory();

  KernelFdt kerlen_fdt2(std::move(kerlen_fdt));

  auto [memory_base, memory_size] = kerlen_fdt2.GetMemory();
  EXPECT_EQ(memory_base, expected_base);
  EXPECT_EQ(memory_size, expected_size);
}

TEST(KernelFdtTest, MoveAssignmentTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
  auto [expected_base, expected_size] = kerlen_fdt.GetMemory();

  KernelFdt kerlen_fdt2;
  kerlen_fdt2 = std::move(kerlen_fdt);

  auto [memory_base, memory_size] = kerlen_fdt2.GetMemory();
  EXPECT_EQ(memory_base, expected_base);
  EXPECT_EQ(memory_size, expected_size);
}
