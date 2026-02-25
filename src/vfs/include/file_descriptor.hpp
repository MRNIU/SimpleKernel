/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 文件描述符表
 */

#ifndef SIMPLEKERNEL_SRC_FS_INCLUDE_FILE_DESCRIPTOR_HPP_
#define SIMPLEKERNEL_SRC_FS_INCLUDE_FILE_DESCRIPTOR_HPP_

#include <cstddef>
#include <cstdint>

#include "expected.hpp"
#include "spinlock.hpp"
#include "vfs.hpp"

namespace vfs {

/**
 * @brief 进程级文件描述符表
 * @details 每个进程（TaskControlBlock）持有一个 FdTable，
 *          将整数 fd 映射到 File 对象。
 *          fd 0/1/2 预留给 stdin/stdout/stderr。
 */
class FileDescriptorTable {
 public:
  /// 最大文件描述符数
  static constexpr int kMaxFd = 64;

  /// 标准文件描述符
  static constexpr int kStdinFd = 0;
  static constexpr int kStdoutFd = 1;
  static constexpr int kStderrFd = 2;

  /// @brief 构造函数
  FileDescriptorTable();

  /// @brief 析构函数
  ~FileDescriptorTable();

  // 禁止拷贝，允许移动
  FileDescriptorTable(const FileDescriptorTable&) = delete;
  FileDescriptorTable(FileDescriptorTable&& other);
  auto operator=(const FileDescriptorTable&) -> FileDescriptorTable& = delete;
  auto operator=(FileDescriptorTable&& other) -> FileDescriptorTable&;

  /**
   * @brief 分配一个最小可用 fd 并关联 File
   * @param file 要关联的 File 对象
   * @return Expected<int> 分配到的 fd
   * @post 返回的 fd >= 0 且 fd < kMaxFd
   */
  auto Alloc(File* file) -> Expected<int>;

  /**
   * @brief 获取 fd 对应的 File 对象
   * @param fd 文件描述符
   * @return File* 指针，无效 fd 返回 nullptr
   * @pre 0 <= fd < kMaxFd
   */
  auto Get(int fd) -> File*;

  /**
   * @brief 释放 fd
   * @param fd 要释放的文件描述符
   * @return Expected<void>
   */
  auto Free(int fd) -> Expected<void>;

  /**
   * @brief 复制文件描述符（用于 dup/dup2）
   * @param old_fd 原文件描述符
   * @param new_fd 目标文件描述符（若为 -1 则分配最小可用）
   * @return Expected<int> 新的文件描述符
   */
  auto Dup(int old_fd, int new_fd = -1) -> Expected<int>;

  /**
   * @brief 关闭所有文件描述符
   * @return Expected<void> 成功或错误
   */
  auto CloseAll() -> Expected<void>;

  /**
   * @brief 设置标准文件描述符
   * @param stdin_file stdin 文件对象
   * @param stdout_file stdout 文件对象
   * @param stderr_file stderr 文件对象
   * @return Expected<void> 成功或错误
   */
  auto SetupStandardFiles(File* stdin_file, File* stdout_file,
                          File* stderr_file) -> Expected<void>;

  /**
   * @brief 获取已打开文件描述符数量
   * @return int 已打开 fd 数量
   */
  [[nodiscard]] auto GetOpenCount() const -> int;

 private:
  File* table_[kMaxFd];
  int open_count_;
  SpinLock lock_;
};

/**
 * @brief 获取当前进程的文件描述符表
 * @return FileDescriptorTable* 当前进程的 fd 表
 * @note 需要在任务管理模块中实现 per-task 存储
 */
auto GetCurrentFdTable() -> FileDescriptorTable*;

/**
 * @brief 设置当前进程的文件描述符表
 * @param fd_table 文件描述符表指针
 */
void SetCurrentFdTable(FileDescriptorTable* fd_table);

}  // namespace vfs

#endif /* SIMPLEKERNEL_SRC_FS_INCLUDE_FILE_DESCRIPTOR_HPP_ */
