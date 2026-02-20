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

TEST_F(KernelFdtTest, ForEachCompatibleNodeTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  size_t count = 0;
  auto result = kerlen_fdt.ForEachCompatibleNode(
      "virtio,mmio",
      [&count](int offset, const char* node_name, uint64_t mmio_base,
               size_t mmio_size, uint32_t irq) -> bool {
        (void)offset;
        (void)node_name;
        (void)mmio_base;
        (void)mmio_size;
        (void)irq;
        ++count;
        return true;
      });
  ASSERT_TRUE(result.has_value()) << result.error().message();
  EXPECT_EQ(count, 8);  // riscv64 virt has 8 virtio,mmio nodes
}

TEST_F(KernelFdtTest, ForEachNodeCompatibleDataTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  bool found_plic = false;
  auto result = kerlen_fdt.ForEachNode(
      [&found_plic](const char* node_name, const char* compatible_data,
                    size_t compatible_len, uint64_t mmio_base, size_t mmio_size,
                    uint32_t irq) -> bool {
        (void)mmio_base;
        (void)mmio_size;
        (void)irq;
        if (compatible_data == nullptr || compatible_len == 0) {
          return true;
        }
        // Look for PLIC node which has multi-string compatible:
        // "sifive,plic-1.0.0\0riscv,plic0"
        if (strcmp(node_name, "plic@c000000") == 0) {
          found_plic = true;
          // Should have both strings in the stringlist
          EXPECT_GT(compatible_len, strlen("sifive,plic-1.0.0") + 1);
          // First string should be "sifive,plic-1.0.0"
          EXPECT_STREQ(compatible_data, "sifive,plic-1.0.0");
          // Second string starts after first null terminator
          const char* second =
              compatible_data + strlen("sifive,plic-1.0.0") + 1;
          EXPECT_STREQ(second, "riscv,plic0");
        }
        return true;
      });
  ASSERT_TRUE(result.has_value()) << result.error().message();
  EXPECT_TRUE(found_plic) << "PLIC node not found in ForEachNode traversal";
}

TEST_F(KernelFdtTest, ForEachCompatibleNodeNoMatchTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  size_t count = 0;
  auto result = kerlen_fdt.ForEachCompatibleNode(
      "nonexistent,device",
      [&count](int offset, const char* node_name, uint64_t mmio_base,
               size_t mmio_size, uint32_t irq) -> bool {
        (void)offset;
        (void)node_name;
        (void)mmio_base;
        (void)mmio_size;
        (void)irq;
        ++count;
        return true;
      });
  ASSERT_TRUE(result.has_value()) << result.error().message();
  EXPECT_EQ(count, 0);  // No matching nodes
}

TEST_F(KernelFdtTest, ForEachCompatibleNodeEarlyStopTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  size_t count = 0;
  auto result = kerlen_fdt.ForEachCompatibleNode(
      "virtio,mmio",
      [&count](int offset, const char* node_name, uint64_t mmio_base,
               size_t mmio_size, uint32_t irq) -> bool {
        (void)offset;
        (void)node_name;
        (void)mmio_base;
        (void)mmio_size;
        (void)irq;
        ++count;
        return count < 3;  // Stop after 3 nodes
      });
  ASSERT_TRUE(result.has_value()) << result.error().message();
  EXPECT_EQ(count, 3);
}

TEST_F(KernelFdtTest, MultiCompatibleMatchTest) {
  KernelFdt kerlen_fdt((uint64_t)riscv64_virt_dtb_data);

  // The PLIC node has compatible = "sifive,plic-1.0.0\0riscv,plic0"
  // ForEachCompatibleNode uses fdt_node_offset_by_compatible which
  // matches against any string in the compatible stringlist
  size_t count = 0;
  auto result = kerlen_fdt.ForEachCompatibleNode(
      "riscv,plic0",
      [&count](int offset, const char* node_name, uint64_t mmio_base,
               size_t mmio_size, uint32_t irq) -> bool {
        (void)offset;
        (void)node_name;
        (void)mmio_base;
        (void)mmio_size;
        (void)irq;
        ++count;
        return true;
      });
  ASSERT_TRUE(result.has_value()) << result.error().message();
  EXPECT_EQ(count,
            1);  // Should find the PLIC node via second compatible string
}
