/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief VFS 核心数据结构和接口
 */

#ifndef SIMPLEKERNEL_SRC_FS_INCLUDE_VFS_HPP_
#define SIMPLEKERNEL_SRC_FS_INCLUDE_VFS_HPP_

#include <cstddef>
#include <cstdint>

#include "expected.hpp"

namespace vfs {

// Forward declarations
struct Inode;
struct Dentry;
struct File;
struct FileSystem;
class MountTable;

/// 文件类型
enum class FileType : uint8_t {
  kUnknown = 0,
  /// 普通文件
  kRegular = 1,
  /// 目录
  kDirectory = 2,
  /// 字符设备
  kCharDevice = 3,
  /// 块设备
  kBlockDevice = 4,
  /// 符号链接
  kSymlink = 5,
  /// 命名管道
  kFifo = 6,
};

/// 文件打开标志（兼容 Linux O_* 定义）
enum OpenFlags : uint32_t {
  kOReadOnly = 0x0000,
  kOWriteOnly = 0x0001,
  kOReadWrite = 0x0002,
  kOCreate = 0x0040,
  kOTruncate = 0x0200,
  kOAppend = 0x0400,
  /// 必须是目录
  kODirectory = 0x010000,
};

/// 文件 seek 基准
enum class SeekWhence : int {
  /// 从文件开头
  kSet = 0,
  /// 从当前位置
  kCur = 1,
  /// 从文件末尾
  kEnd = 2,
};

/// Inode 操作接口
struct InodeOps {
  /**
   * @brief 在目录中查找指定名称的 inode
   * @param dir 父目录 inode
   * @param name 要查找的文件名
   * @return Expected<Inode*> 找到的 inode 或错误
   * @pre dir != nullptr && dir->type == FileType::kDirectory
   * @pre name != nullptr && strlen(name) > 0
   */
  auto (*lookup)(Inode* dir, const char* name) -> Expected<Inode*>;

  /**
   * @brief 在目录中创建新文件
   * @param dir 父目录 inode
   * @param name 文件名
   * @param type 文件类型
   * @return Expected<Inode*> 新创建的 inode 或错误
   * @pre dir != nullptr && dir->type == FileType::kDirectory
   * @pre name != nullptr && strlen(name) > 0
   * @post 返回的 inode->type == type
   */
  auto (*create)(Inode* dir, const char* name, FileType type)
      -> Expected<Inode*>;

  /**
   * @brief 删除文件（解除链接）
   * @param dir 父目录 inode
   * @param name 要删除的文件名
   * @return Expected<void> 成功或错误
   * @pre dir != nullptr && dir->type == FileType::kDirectory
   * @pre name != nullptr
   */
  auto (*unlink)(Inode* dir, const char* name) -> Expected<void>;

  /**
   * @brief 创建目录
   * @param dir 父目录 inode
   * @param name 目录名
   * @return Expected<Inode*> 新创建的目录 inode 或错误
   * @pre dir != nullptr && dir->type == FileType::kDirectory
   * @pre name != nullptr && strlen(name) > 0
   * @post 返回的 inode->type == FileType::kDirectory
   */
  auto (*mkdir)(Inode* dir, const char* name) -> Expected<Inode*>;

  /**
   * @brief 删除目录
   * @param dir 父目录 inode
   * @param name 要删除的目录名
   * @return Expected<void> 成功或错误
   * @pre dir != nullptr && dir->type == FileType::kDirectory
   * @pre name != nullptr
   */
  auto (*rmdir)(Inode* dir, const char* name) -> Expected<void>;
};

/**
 * @brief Inode — 文件元数据（独立于路径名）
 * @details 每个文件/目录在 VFS 中有且仅有一个 Inode。
 *          Inode 持有文件的元信息和操作方法指针。
 */
struct Inode {
  /// inode 编号（文件系统内唯一）
  uint64_t ino;
  /// 文件类型
  FileType type;
  /// 文件大小（字节）
  uint64_t size;
  /// 权限位（简化版）
  uint32_t permissions;
  /// 硬链接计数
  uint32_t link_count;
  /// 文件系统私有数据指针
  void* fs_private;
  /// 所属文件系统
  FileSystem* fs;

  /// inode 操作接口
  InodeOps* ops = nullptr;

  /// @brief 构造函数
  Inode()
      : ino(0),
        type(FileType::kUnknown),
        size(0),
        permissions(0644),
        link_count(1),
        fs_private(nullptr),
        fs(nullptr),
        ops(nullptr) {}
};

/**
 * @brief Dentry — 目录项缓存（路径名 ↔ Inode 的映射）
 * @details Dentry 构成一棵树，反映目录层次结构。
 *          支持路径查找加速。
 */
struct Dentry {
  /// 文件/目录名
  char name[256];
  /// 关联的 inode
  Inode* inode;
  /// 父目录项
  Dentry* parent;
  /// 子目录项链表头
  Dentry* children;
  /// 兄弟目录项（同一父目录下）
  Dentry* next_sibling;
  /// 文件系统私有数据
  void* fs_private;

  /// @brief 构造函数
  Dentry()
      : inode(nullptr),
        parent(nullptr),
        children(nullptr),
        next_sibling(nullptr),
        fs_private(nullptr) {
    name[0] = '\0';
  }
};

/// 目录项结构（用于 readdir）
struct DirEntry {
  /// inode 编号
  uint64_t ino;
  /// 文件类型
  uint8_t type;
  /// 文件名
  char name[256];
};

/// File 操作接口
struct FileOps {
  /**
   * @brief 从文件读取数据
   * @param file 文件对象
   * @param buf 输出缓冲区
   * @param count 最大读取字节数
   * @return Expected<size_t> 实际读取的字节数或错误
   * @pre file != nullptr && buf != nullptr
   * @post 返回值 <= count
   */
  auto (*read)(File* file, void* buf, size_t count) -> Expected<size_t>;

  /**
   * @brief 向文件写入数据
   * @param file 文件对象
   * @param buf 输入缓冲区
   * @param count 要写入的字节数
   * @return Expected<size_t> 实际写入的字节数或错误
   * @pre file != nullptr && buf != nullptr
   * @post 返回值 <= count
   */
  auto (*write)(File* file, const void* buf, size_t count) -> Expected<size_t>;

  /**
   * @brief 调整文件偏移量
   * @param file 文件对象
   * @param offset 偏移量
   * @param whence 基准位置
   * @return Expected<uint64_t> 新的偏移量或错误
   * @pre file != nullptr
   */
  auto (*seek)(File* file, int64_t offset, SeekWhence whence)
      -> Expected<uint64_t>;

  /**
   * @brief 关闭文件
   * @param file 文件对象
   * @return Expected<void> 成功或错误
   * @pre file != nullptr
   * @post file 对象将被释放
   */
  auto (*close)(File* file) -> Expected<void>;

  /**
   * @brief 读取目录项
   * @param file 目录文件对象
   * @param dirent 输出目录项数组
   * @param count 最多读取的条目数
   * @return Expected<size_t> 实际读取的条目数或错误
   * @pre file != nullptr && file->inode->type == FileType::kDirectory
   * @pre dirent != nullptr
   */
  auto (*readdir)(File* file, DirEntry* dirent, size_t count)
      -> Expected<size_t>;
};

/**
 * @brief File — 打开的文件实例（每次 open 产生一个）
 * @details File 对象持有当前偏移量和操作方法指针。
 *          多个 File 可以指向同一个 Inode。
 */
struct File {
  /// 关联的 inode
  Inode* inode;
  /// 关联的 dentry
  Dentry* dentry;
  /// 当前读写偏移量
  uint64_t offset;
  /// 打开标志 (OpenFlags)
  uint32_t flags;

  /// 文件操作接口
  FileOps* ops = nullptr;

  /// @brief 构造函数
  File() : inode(nullptr), dentry(nullptr), offset(0), flags(0), ops(nullptr) {}
};

/**
 * @brief VFS 全局初始化
 * @return Expected<void> 成功或错误
 * @post VFS 子系统已准备好接受挂载请求
 */
auto Init() -> Expected<void>;

/**
 * @brief 路径解析，查找 dentry
 * @param path 绝对路径（以 / 开头）
 * @return Expected<Dentry*> 找到的 dentry 或错误
 * @pre path != nullptr && path[0] == '/'
 */
auto Lookup(const char* path) -> Expected<Dentry*>;

/**
 * @brief 打开文件
 * @param path 文件路径
 * @param flags 打开标志
 * @return Expected<File*> 文件对象或错误
 * @pre path != nullptr
 * @post 成功时返回有效的 File 对象
 */
auto Open(const char* path, uint32_t flags) -> Expected<File*>;

/**
 * @brief 关闭文件
 * @param file 文件对象
 * @return Expected<void> 成功或错误
 * @pre file != nullptr
 */
auto Close(File* file) -> Expected<void>;

/**
 * @brief 从文件读取数据
 * @param file 文件对象
 * @param buf 输出缓冲区
 * @param count 最大读取字节数
 * @return Expected<size_t> 实际读取的字节数或错误
 * @pre file != nullptr && buf != nullptr
 */
auto Read(File* file, void* buf, size_t count) -> Expected<size_t>;

/**
 * @brief 向文件写入数据
 * @param file 文件对象
 * @param buf 输入缓冲区
 * @param count 要写入的字节数
 * @return Expected<size_t> 实际写入的字节数或错误
 * @pre file != nullptr && buf != nullptr
 */
auto Write(File* file, const void* buf, size_t count) -> Expected<size_t>;

/**
 * @brief 调整文件偏移量
 * @param file 文件对象
 * @param offset 偏移量
 * @param whence 基准位置
 * @return Expected<uint64_t> 新的偏移量或错误
 * @pre file != nullptr
 */
auto Seek(File* file, int64_t offset, SeekWhence whence) -> Expected<uint64_t>;

/**
 * @brief 创建目录
 * @param path 目录路径
 * @return Expected<void> 成功或错误
 * @pre path != nullptr
 */
auto MkDir(const char* path) -> Expected<void>;

/**
 * @brief 删除目录
 * @param path 目录路径
 * @return Expected<void> 成功或错误
 * @pre path != nullptr
 */
auto RmDir(const char* path) -> Expected<void>;

/**
 * @brief 删除文件
 * @param path 文件路径
 * @return Expected<void> 成功或错误
 * @pre path != nullptr
 */
auto Unlink(const char* path) -> Expected<void>;

/**
 * @brief 读取目录内容
 * @param file 目录文件对象
 * @param dirent 输出目录项数组
 * @param count 最多读取的条目数
 * @return Expected<size_t> 实际读取的条目数或错误
 * @pre file != nullptr && file->inode->type == FileType::kDirectory
 */
auto ReadDir(File* file, DirEntry* dirent, size_t count) -> Expected<size_t>;

/**
 * @brief 获取根目录 dentry
 * @return Dentry* 根目录 dentry
 * @pre VFS 已初始化且根文件系统已挂载
 */
auto GetRootDentry() -> Dentry*;

}  // namespace vfs

#endif  // SIMPLEKERNEL_SRC_FS_INCLUDE_VFS_HPP_
