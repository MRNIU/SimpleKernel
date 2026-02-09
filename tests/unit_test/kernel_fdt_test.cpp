/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kernel_fdt.hpp"

#include <gtest/gtest.h>

#include "riscv64_virt.dtb.h"
#include "test_environment_state.hpp"

class KernelFdtTest : public ::testing::Test {
 protected:
  void SetUp() override {
    env_state_.InitializeCores(1);
    env_state_.SetCurrentThreadEnvironment();
    env_state_.BindThreadToCore(std::this_thread::get_id(), 0);
  }

  void TearDown() override { env_state_.ClearCurrentThreadEnvironment(); }

  test_env::TestEnvironmentState env_state_;
};

TEST_F(KernelFdtTest, ConstructorTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
}

TEST_F(KernelFdtTest, GetMemoryTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  auto result = kerlen_fdt.GetMemory();
  ASSERT_TRUE(result.has_value()) << result.error().message();

  auto [memory_base, memory_size] = *result;
  EXPECT_EQ(memory_base, 0x80000000);
  EXPECT_EQ(memory_size, 0x8000000);
}

TEST_F(KernelFdtTest, GetSerialTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  auto result = kerlen_fdt.GetSerial();
  ASSERT_TRUE(result.has_value()) << result.error().message();

  auto [serial_base, serial_size, serial_irq] = *result;
  EXPECT_EQ(serial_base, 0x10000000);
  EXPECT_EQ(serial_size, 0x100);
  EXPECT_EQ(serial_irq, 0xA);
}

TEST_F(KernelFdtTest, GetTimebaseFrequencyTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  auto result = kerlen_fdt.GetTimebaseFrequency();
  ASSERT_TRUE(result.has_value()) << result.error().message();

  EXPECT_EQ(*result, 0x989680);
}

TEST_F(KernelFdtTest, GetCoreCountTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  auto result = kerlen_fdt.GetCoreCount();
  ASSERT_TRUE(result.has_value()) << result.error().message();

  EXPECT_GT(*result, 0);  // 至少有一个 CPU 核心
}

TEST_F(KernelFdtTest, CopyConstructorTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
  KernelFdt kerlen_fdt2(kerlen_fdt);

  auto result1 = kerlen_fdt.GetMemory();
  auto result2 = kerlen_fdt2.GetMemory();

  ASSERT_TRUE(result1.has_value());
  ASSERT_TRUE(result2.has_value());

  auto [memory_base1, memory_size1] = *result1;
  auto [memory_base2, memory_size2] = *result2;

  EXPECT_EQ(memory_base1, memory_base2);
  EXPECT_EQ(memory_size1, memory_size2);
}

TEST_F(KernelFdtTest, AssignmentTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
  KernelFdt kerlen_fdt2;

  kerlen_fdt2 = kerlen_fdt;

  auto result1 = kerlen_fdt.GetMemory();
  auto result2 = kerlen_fdt2.GetMemory();

  ASSERT_TRUE(result1.has_value());
  ASSERT_TRUE(result2.has_value());

  auto [memory_base1, memory_size1] = *result1;
  auto [memory_base2, memory_size2] = *result2;

  EXPECT_EQ(memory_base1, memory_base2);
  EXPECT_EQ(memory_size1, memory_size2);
}

TEST_F(KernelFdtTest, MoveConstructorTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
  auto result = kerlen_fdt.GetMemory();
  ASSERT_TRUE(result.has_value());
  auto [expected_base, expected_size] = *result;

  KernelFdt kerlen_fdt2(std::move(kerlen_fdt));

  auto result2 = kerlen_fdt2.GetMemory();
  ASSERT_TRUE(result2.has_value());
  auto [memory_base, memory_size] = *result2;

  EXPECT_EQ(memory_base, expected_base);
  EXPECT_EQ(memory_size, expected_size);
}

TEST_F(KernelFdtTest, MoveAssignmentTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);
  auto result = kerlen_fdt.GetMemory();
  ASSERT_TRUE(result.has_value());
  auto [expected_base, expected_size] = *result;

  KernelFdt kerlen_fdt2;
  kerlen_fdt2 = std::move(kerlen_fdt);

  auto result2 = kerlen_fdt2.GetMemory();
  ASSERT_TRUE(result2.has_value());
  auto [memory_base, memory_size] = *result2;

  EXPECT_EQ(memory_base, expected_base);
  EXPECT_EQ(memory_size, expected_size);
}
