/** @copyright Copyright The SimpleKernel Contributors */

#include "virtio/virtio_driver.hpp"

#include <gtest/gtest.h>

TEST(VirtioDriverTest, GetEntryNameIsVirtio) {
  const auto& entry = VirtioDriver::GetEntry();
  EXPECT_STREQ(entry.name, "virtio");
}

TEST(VirtioDriverTest, MatchStaticReturnsFalseWhenNoMmioBase) {
  DeviceNode node{};
  node.mmio_base = 0;
  EXPECT_FALSE(VirtioDriver::MatchStatic(node));
}

TEST(VirtioDriverTest, MatchTableContainsVirtioMmio) {
  const auto& entry = VirtioDriver::GetEntry();
  bool found = false;
  for (const auto& m : entry.match_table) {
    if (m.bus_type == BusType::kPlatform &&
        __builtin_strcmp(m.compatible, "virtio,mmio") == 0) {
      found = true;
    }
  }
  EXPECT_TRUE(found);
}
