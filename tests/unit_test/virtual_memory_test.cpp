/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "virtual_memory.hpp"

#include <gmock/gmock.h>
#include <gtest/gtest.h>

#include <cstdint>
#include <cstring>
#include <unordered_map>
#include <vector>

namespace {

// Mock allocator for testing
class MockAllocator {
 public:
  static auto GetInstance() -> MockAllocator& {
    static MockAllocator instance;
    return instance;
  }

  auto AlignedAlloc(size_t alignment, size_t size) -> void* {
    void* ptr = nullptr;
    if (posix_memalign(&ptr, alignment, size) == 0) {
      allocated_blocks_[ptr] = size;
      return ptr;
    }
    return nullptr;
  }

  void Free(void* ptr) {
    if (ptr == nullptr) {
      return;
    }
    auto it = allocated_blocks_.find(ptr);
    if (it != allocated_blocks_.end()) {
      allocated_blocks_.erase(it);
      free(ptr);
    }
  }

  void Reset() {
    for (auto& [ptr, size] : allocated_blocks_) {
      free(ptr);
    }
    allocated_blocks_.clear();
  }

  [[nodiscard]] auto GetAllocatedCount() const -> size_t {
    return allocated_blocks_.size();
  }

 private:
  MockAllocator() = default;
  std::unordered_map<void*, size_t> allocated_blocks_;
};

// C-style wrapper functions
extern "C" void* test_aligned_alloc(size_t alignment, size_t size) {
  return MockAllocator::GetInstance().AlignedAlloc(alignment, size);
}

extern "C" void test_free(void* ptr) { MockAllocator::GetInstance().Free(ptr); }

class VirtualMemoryTest : public ::testing::Test {
 protected:
  void SetUp() override {
    Singleton<BasicInfo>::GetInstance().physical_memory_addr = 0x80000000;
    Singleton<BasicInfo>::GetInstance().physical_memory_size = 0x10000000;
    MockAllocator::GetInstance().Reset();
  }

  void TearDown() override { MockAllocator::GetInstance().Reset(); }
};

TEST_F(VirtualMemoryTest, MapPageBasic) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* page_dir = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                      cpu_io::virtual_memory::kPageSize);
  ASSERT_NE(page_dir, nullptr);
  std::memset(page_dir, 0, cpu_io::virtual_memory::kPageSize);

  void* virt_addr = reinterpret_cast<void*>(0x1000);
  void* phys_addr = reinterpret_cast<void*>(0x80001000);

  // 映射页面
  bool result = vm.MapPage(page_dir, virt_addr, phys_addr,
                           cpu_io::virtual_memory::GetUserPagePermissions());

  EXPECT_TRUE(result);

  // 验证映射
  auto mapped = vm.GetMapping(page_dir, virt_addr);
  EXPECT_TRUE(mapped.has_value());
  if (mapped) {
    EXPECT_EQ(*mapped, phys_addr);
  }
}

TEST_F(VirtualMemoryTest, UnmapPage) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* page_dir = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                      cpu_io::virtual_memory::kPageSize);
  ASSERT_NE(page_dir, nullptr);
  std::memset(page_dir, 0, cpu_io::virtual_memory::kPageSize);

  void* virt_addr = reinterpret_cast<void*>(0x1000);
  void* phys_addr = reinterpret_cast<void*>(0x80001000);

  // 映射页面
  vm.MapPage(page_dir, virt_addr, phys_addr,
             cpu_io::virtual_memory::GetUserPagePermissions());

  // 取消映射
  bool result = vm.UnmapPage(page_dir, virt_addr);
  EXPECT_TRUE(result);

  // 验证取消映射
  auto mapped = vm.GetMapping(page_dir, virt_addr);
  EXPECT_FALSE(mapped.has_value());
}

TEST_F(VirtualMemoryTest, UnmapNonExistentPage) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* page_dir = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                      cpu_io::virtual_memory::kPageSize);
  ASSERT_NE(page_dir, nullptr);
  std::memset(page_dir, 0, cpu_io::virtual_memory::kPageSize);

  void* virt_addr = reinterpret_cast<void*>(0x1000);

  // 取消映射不存在的页面
  bool result = vm.UnmapPage(page_dir, virt_addr);
  EXPECT_FALSE(result);
}

TEST_F(VirtualMemoryTest, GetMappingNonExistent) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* page_dir = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                      cpu_io::virtual_memory::kPageSize);
  ASSERT_NE(page_dir, nullptr);
  std::memset(page_dir, 0, cpu_io::virtual_memory::kPageSize);

  void* virt_addr = reinterpret_cast<void*>(0x1000);

  auto mapped = vm.GetMapping(page_dir, virt_addr);
  EXPECT_FALSE(mapped.has_value());
}

TEST_F(VirtualMemoryTest, MapMultiplePages) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* page_dir = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                      cpu_io::virtual_memory::kPageSize);
  ASSERT_NE(page_dir, nullptr);
  std::memset(page_dir, 0, cpu_io::virtual_memory::kPageSize);

  constexpr size_t kNumPages = 10;
  constexpr size_t kPageSize = 0x1000;

  // 映射多个页面
  for (size_t i = 0; i < kNumPages; ++i) {
    void* virt_addr = reinterpret_cast<void*>(0x10000 + i * kPageSize);
    void* phys_addr = reinterpret_cast<void*>(0x80000000 + i * kPageSize);

    bool result = vm.MapPage(page_dir, virt_addr, phys_addr,
                             cpu_io::virtual_memory::GetUserPagePermissions());
    EXPECT_TRUE(result);
  }

  // 验证所有映射
  for (size_t i = 0; i < kNumPages; ++i) {
    void* virt_addr = reinterpret_cast<void*>(0x10000 + i * kPageSize);
    void* phys_addr = reinterpret_cast<void*>(0x80000000 + i * kPageSize);

    auto mapped = vm.GetMapping(page_dir, virt_addr);
    EXPECT_TRUE(mapped.has_value());
    if (mapped) {
      EXPECT_EQ(*mapped, phys_addr);
    }
  }
}

TEST_F(VirtualMemoryTest, RemapPage) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* page_dir = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                      cpu_io::virtual_memory::kPageSize);
  ASSERT_NE(page_dir, nullptr);
  std::memset(page_dir, 0, cpu_io::virtual_memory::kPageSize);

  void* virt_addr = reinterpret_cast<void*>(0x1000);
  void* phys_addr1 = reinterpret_cast<void*>(0x80001000);
  void* phys_addr2 = reinterpret_cast<void*>(0x80002000);

  // 第一次映射
  vm.MapPage(page_dir, virt_addr, phys_addr1,
             cpu_io::virtual_memory::GetUserPagePermissions());

  // 重新映射到不同的物理地址
  bool result = vm.MapPage(page_dir, virt_addr, phys_addr2,
                           cpu_io::virtual_memory::GetUserPagePermissions());
  EXPECT_TRUE(result);

  // 验证新映射
  auto mapped = vm.GetMapping(page_dir, virt_addr);
  EXPECT_TRUE(mapped.has_value());
  if (mapped) {
    EXPECT_EQ(*mapped, phys_addr2);
  }
}

TEST_F(VirtualMemoryTest, DestroyPageDirectoryWithoutFreePages) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* page_dir = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                      cpu_io::virtual_memory::kPageSize);
  ASSERT_NE(page_dir, nullptr);
  std::memset(page_dir, 0, cpu_io::virtual_memory::kPageSize);

  // 映射一些页面
  constexpr size_t kNumPages = 5;
  constexpr size_t kPageSize = 0x1000;

  for (size_t i = 0; i < kNumPages; ++i) {
    void* virt_addr = reinterpret_cast<void*>(0x10000 + i * kPageSize);
    void* phys_addr = reinterpret_cast<void*>(0x80000000 + i * kPageSize);
    vm.MapPage(page_dir, virt_addr, phys_addr,
               cpu_io::virtual_memory::GetUserPagePermissions());
  }

  size_t allocated_before = MockAllocator::GetInstance().GetAllocatedCount();

  // 销毁页表（不释放物理页）
  vm.DestroyPageDirectory(page_dir, false);

  size_t allocated_after = MockAllocator::GetInstance().GetAllocatedCount();

  // 验证页表已释放
  EXPECT_LT(allocated_after, allocated_before);
}

TEST_F(VirtualMemoryTest, ClonePageDirectoryWithMappings) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* src_page_dir = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                          cpu_io::virtual_memory::kPageSize);
  ASSERT_NE(src_page_dir, nullptr);
  std::memset(src_page_dir, 0, cpu_io::virtual_memory::kPageSize);

  // 在源页表中映射页面
  constexpr size_t kNumPages = 5;
  constexpr size_t kPageSize = 0x1000;

  for (size_t i = 0; i < kNumPages; ++i) {
    void* virt_addr = reinterpret_cast<void*>(0x10000 + i * kPageSize);
    void* phys_addr = reinterpret_cast<void*>(0x80000000 + i * kPageSize);
    vm.MapPage(src_page_dir, virt_addr, phys_addr,
               cpu_io::virtual_memory::GetUserPagePermissions());
  }

  // 克隆页表（复制映射）
  auto* dst_page_dir = vm.ClonePageDirectory(src_page_dir, true);
  ASSERT_NE(dst_page_dir, nullptr);
  EXPECT_NE(dst_page_dir, src_page_dir);

  // 验证克隆的页表具有相同的映射
  for (size_t i = 0; i < kNumPages; ++i) {
    void* virt_addr = reinterpret_cast<void*>(0x10000 + i * kPageSize);
    void* phys_addr = reinterpret_cast<void*>(0x80000000 + i * kPageSize);

    auto src_mapped = vm.GetMapping(src_page_dir, virt_addr);
    auto dst_mapped = vm.GetMapping(dst_page_dir, virt_addr);

    EXPECT_TRUE(src_mapped.has_value());
    EXPECT_TRUE(dst_mapped.has_value());

    if (src_mapped && dst_mapped) {
      EXPECT_EQ(*src_mapped, phys_addr);
      EXPECT_EQ(*dst_mapped, phys_addr);
      EXPECT_EQ(*src_mapped, *dst_mapped);
    }
  }

  // 清理
  vm.DestroyPageDirectory(src_page_dir, false);
  vm.DestroyPageDirectory(dst_page_dir, false);
}

TEST_F(VirtualMemoryTest, ClonePageDirectoryWithoutMappings) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* src_page_dir = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                          cpu_io::virtual_memory::kPageSize);
  ASSERT_NE(src_page_dir, nullptr);
  std::memset(src_page_dir, 0, cpu_io::virtual_memory::kPageSize);

  // 在源页表中映射页面
  constexpr size_t kNumPages = 3;
  constexpr size_t kPageSize = 0x1000;

  for (size_t i = 0; i < kNumPages; ++i) {
    void* virt_addr = reinterpret_cast<void*>(0x10000 + i * kPageSize);
    void* phys_addr = reinterpret_cast<void*>(0x80000000 + i * kPageSize);
    vm.MapPage(src_page_dir, virt_addr, phys_addr,
               cpu_io::virtual_memory::GetUserPagePermissions());
  }

  // 克隆页表（不复制映射）
  auto* dst_page_dir = vm.ClonePageDirectory(src_page_dir, false);
  ASSERT_NE(dst_page_dir, nullptr);
  EXPECT_NE(dst_page_dir, src_page_dir);

  // 验证克隆的页表没有映射
  for (size_t i = 0; i < kNumPages; ++i) {
    void* virt_addr = reinterpret_cast<void*>(0x10000 + i * kPageSize);

    auto src_mapped = vm.GetMapping(src_page_dir, virt_addr);
    auto dst_mapped = vm.GetMapping(dst_page_dir, virt_addr);

    EXPECT_TRUE(src_mapped.has_value());
    EXPECT_FALSE(dst_mapped.has_value());
  }

  // 清理
  vm.DestroyPageDirectory(src_page_dir, false);
  vm.DestroyPageDirectory(dst_page_dir, false);
}

TEST_F(VirtualMemoryTest, CloneNullPageDirectory) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* dst_page_dir = vm.ClonePageDirectory(nullptr, true);
  EXPECT_EQ(dst_page_dir, nullptr);
}

TEST_F(VirtualMemoryTest, DestroyNullPageDirectory) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  // 应该不会崩溃
  vm.DestroyPageDirectory(nullptr, false);
  vm.DestroyPageDirectory(nullptr, true);
}

TEST_F(VirtualMemoryTest, MemoryLeakCheck) {
  VirtualMemory vm(test_aligned_alloc, test_free);

  auto* page_dir1 = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                       cpu_io::virtual_memory::kPageSize);
  auto* page_dir2 = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                       cpu_io::virtual_memory::kPageSize);
  auto* page_dir3 = test_aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                       cpu_io::virtual_memory::kPageSize);

  ASSERT_NE(page_dir1, nullptr);
  ASSERT_NE(page_dir2, nullptr);
  ASSERT_NE(page_dir3, nullptr);
  std::memset(page_dir1, 0, cpu_io::virtual_memory::kPageSize);
  std::memset(page_dir2, 0, cpu_io::virtual_memory::kPageSize);
  std::memset(page_dir3, 0, cpu_io::virtual_memory::kPageSize);

  size_t allocated_before = MockAllocator::GetInstance().GetAllocatedCount();

  // 映射一些页面
  for (size_t i = 0; i < 10; ++i) {
    void* virt_addr = reinterpret_cast<void*>(0x10000 + i * 0x1000);
    void* phys_addr = reinterpret_cast<void*>(0x80000000 + i * 0x1000);
    vm.MapPage(page_dir1, virt_addr, phys_addr,
               cpu_io::virtual_memory::GetUserPagePermissions());
  }

  // 克隆
  auto* cloned = vm.ClonePageDirectory(page_dir1, true);
  ASSERT_NE(cloned, nullptr);

  // 清理所有页表
  vm.DestroyPageDirectory(page_dir1, false);
  vm.DestroyPageDirectory(page_dir2, false);
  vm.DestroyPageDirectory(page_dir3, false);
  vm.DestroyPageDirectory(cloned, false);

  // 应该释放了大部分页表内存（除了内核页表）
  size_t allocated_after = MockAllocator::GetInstance().GetAllocatedCount();
  EXPECT_LT(allocated_after, allocated_before);
}

}  // namespace
