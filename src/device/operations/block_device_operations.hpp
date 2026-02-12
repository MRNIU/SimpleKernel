/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_OPERATIONS_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_OPERATIONS_HPP_

#include "device_operations_base.hpp"

/**
 * @brief 块设备抽象接口
 *
 * 以固定大小的扇区为最小读写单位，支持随机访问。
 * 新增 ReadBlocks/WriteBlocks、Flush 和容量查询接口。
 * 基类的字节级 Read/Write 会自动桥接为块操作（要求对齐）。
 *
 * @tparam Derived 具体块设备类型
 *
 * @pre  派生类必须实现 DoGetBlockSize 和 DoGetBlockCount
 * @pre  派生类至少实现 DoReadBlocks 或 DoWriteBlocks 之一
 */
template <class Derived>
class BlockDevice : public DeviceOperationsBase<Derived> {
 public:
  /**
   * @brief 从设备读取指定数量的块
   *
   * @param  block_no     起始块号（0-based）
   * @param  buffer       目标缓冲区，大小必须 >= block_count * GetBlockSize()
   * @param  block_count  要读取的块数
   * @return Expected<size_t> 实际读取的块数
   */
  auto ReadBlocks(this Derived& self, uint64_t block_no,
                  std::span<uint8_t> buffer, size_t block_count)
      -> Expected<size_t> {
    auto check = self.ValidateBlockAccess(block_no, buffer.size(), block_count);
    if (!check) {
      return std::unexpected(check.error());
    }
    return self.DoReadBlocks(block_no, buffer, block_count);
  }

  /**
   * @brief 向设备写入指定数量的块
   *
   * @param  block_no     起始块号（0-based）
   * @param  data         待写入数据，大小必须 >= block_count * GetBlockSize()
   * @param  block_count  要写入的块数
   * @return Expected<size_t> 实际写入的块数
   */
  auto WriteBlocks(this Derived& self, uint64_t block_no,
                   std::span<const uint8_t> data, size_t block_count)
      -> Expected<size_t> {
    auto check = self.ValidateBlockAccess(block_no, data.size(), block_count);
    if (!check) {
      return std::unexpected(check.error());
    }
    return self.DoWriteBlocks(block_no, data, block_count);
  }

  /**
   * @brief 读取单个块
   */
  auto ReadBlock(this Derived& self, uint64_t block_no,
                 std::span<uint8_t> buffer) -> Expected<void> {
    auto result = self.ReadBlocks(block_no, buffer, 1);
    if (!result) {
      return std::unexpected(result.error());
    }
    return {};
  }

  /**
   * @brief 写入单个块
   */
  auto WriteBlock(this Derived& self, uint64_t block_no,
                  std::span<const uint8_t> data) -> Expected<void> {
    auto result = self.WriteBlocks(block_no, data, 1);
    if (!result) {
      return std::unexpected(result.error());
    }
    return {};
  }

  /**
   * @brief 将缓存数据刷写到持久存储
   */
  auto Flush(this Derived& self) -> Expected<void> { return self.DoFlush(); }

  /**
   * @brief 获取块大小（字节数）
   */
  auto GetBlockSize(this const Derived& self) -> size_t {
    return self.DoGetBlockSize();
  }

  /**
   * @brief 获取设备总块数
   */
  auto GetBlockCount(this const Derived& self) -> uint64_t {
    return self.DoGetBlockCount();
  }

  /**
   * @brief 获取设备总容量（字节）
   */
  auto GetCapacity(this const Derived& self) -> uint64_t {
    return self.GetBlockSize() * self.GetBlockCount();
  }

 protected:
  /**
   * @brief 块读取实现（派生类覆写）
   */
  auto DoReadBlocks([[maybe_unused]] uint64_t block_no,
                    [[maybe_unused]] std::span<uint8_t> buffer,
                    [[maybe_unused]] size_t block_count) -> Expected<size_t> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief 块写入实现（派生类覆写）
   */
  auto DoWriteBlocks([[maybe_unused]] uint64_t block_no,
                     [[maybe_unused]] std::span<const uint8_t> data,
                     [[maybe_unused]] size_t block_count) -> Expected<size_t> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief Flush 实现（派生类覆写）
   */
  auto DoFlush() -> Expected<void> {
    return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
  }

  /**
   * @brief 获取块大小实现（派生类必须覆写）
   * @note  默认返回 512（最常见的扇区大小）
   */
  auto DoGetBlockSize() const -> size_t { return 512; }

  /**
   * @brief 获取块总数实现（派生类必须覆写）
   * @note  默认返回 0（表示未初始化）
   */
  auto DoGetBlockCount() const -> uint64_t { return 0; }

  /**
   * @brief 字节级读取 → 块读取的桥接（要求对齐）
   */
  auto DoRead(this Derived& self, std::span<uint8_t> buffer, size_t offset)
      -> Expected<size_t> {
    size_t block_size = self.DoGetBlockSize();
    if (block_size == 0) {
      return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
    }
    if (offset % block_size != 0 || buffer.size() % block_size != 0) {
      return std::unexpected(Error{ErrorCode::kDeviceBlockUnaligned});
    }
    uint64_t block_no = offset / block_size;
    size_t block_count = buffer.size() / block_size;
    auto result = self.DoReadBlocks(block_no, buffer, block_count);
    if (!result) {
      return std::unexpected(result.error());
    }
    return *result * block_size;
  }

  /**
   * @brief 字节级写入 → 块写入的桥接（要求对齐）
   */
  auto DoWrite(this Derived& self, std::span<const uint8_t> data, size_t offset)
      -> Expected<size_t> {
    size_t block_size = self.DoGetBlockSize();
    if (block_size == 0) {
      return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
    }
    if (offset % block_size != 0 || data.size() % block_size != 0) {
      return std::unexpected(Error{ErrorCode::kDeviceBlockUnaligned});
    }
    uint64_t block_no = offset / block_size;
    size_t block_count = data.size() / block_size;
    auto result = self.DoWriteBlocks(block_no, data, block_count);
    if (!result) {
      return std::unexpected(result.error());
    }
    return *result * block_size;
  }

 private:
  /**
   * @brief 校验块访问参数的合法性
   * @param  block_no     起始块号
   * @param  buffer_size  数据缓冲区大小（字节）
   * @param  block_count  块数量
   * @return Expected<void> 参数合法返回 void，非法返回错误码
   */
  auto ValidateBlockAccess(this const Derived& self, uint64_t block_no,
                           size_t buffer_size, size_t block_count)
      -> Expected<void> {
    size_t block_size = self.GetBlockSize();

    if (block_size == 0) {
      return std::unexpected(Error{ErrorCode::kDeviceNotSupported});
    }
    if (buffer_size < block_count * block_size) {
      return std::unexpected(Error{ErrorCode::kInvalidArgument});
    }
    if (block_no + block_count > self.GetBlockCount()) {
      return std::unexpected(Error{ErrorCode::kDeviceBlockOutOfRange});
    }
    return {};
  }
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_BLOCK_DEVICE_OPERATIONS_HPP_ */
