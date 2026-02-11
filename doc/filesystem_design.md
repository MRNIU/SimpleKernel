# SimpleKernel 文件系统设计规划

## 1. 概述

本文档规划 SimpleKernel 的文件系统子系统，包含以下 5 个模块，按依赖关系自底向上实现：

```
┌─────────────────────────────────────────────────┐
│              用户接口 (syscall)                    │
│     open / close / read / write / mount          │
├─────────────────────────────────────────────────┤
│                VFS 层                             │
│  Superblock · Inode · Dentry · File · MountTable │
├──────────────────────┬──────────────────────────┤
│       ramfs          │         fat32             │
├──────────────────────┴──────────────────────────┤
│              块设备抽象层                          │
│            BlockDevice 接口                       │
├─────────────────────────────────────────────────┤
│          virtio-blk 驱动                          │
│     MMIO transport · VirtQueue · 请求处理         │
└─────────────────────────────────────────────────┘
```

### 实现优先级与依赖链

| 阶段 | 模块 | 依赖 | 产出 |
|------|------|------|------|
| P0 | 块设备接口 + virtio-blk 驱动 | FDT 解析、MMIO、中断 | 能读写 QEMU 磁盘扇区 |
| P1 | VFS 框架 | 无外部依赖 | inode/dentry/file 抽象 |
| P2 | ramfs | VFS | 内存文件系统，验证 VFS 正确性 |
| P3 | FAT32 | VFS + 块设备 | 能读写 QEMU 磁盘上的 FAT32 分区 |
| P4 | 系统调用集成 | VFS + 任务管理 | open/close/read/write/mount |

---

## 2. 目录结构

```
src/
├── fs/                              # 📁 新增：文件系统子系统
│   ├── CMakeLists.txt
│   ├── include/                     # 📐 接口头文件
│   │   ├── block_device.hpp         # 块设备抽象接口
│   │   ├── vfs.hpp                  # VFS 核心数据结构
│   │   ├── filesystem.hpp           # 文件系统基类接口
│   │   ├── file_descriptor.hpp      # 文件描述符表
│   │   └── mount.hpp                # 挂载管理
│   ├── vfs.cpp                      # VFS 实现
│   ├── file_descriptor.cpp          # 文件描述符表实现
│   ├── mount.cpp                    # 挂载管理实现
│   ├── ramfs/                       # ramfs 文件系统
│   │   ├── CMakeLists.txt
│   │   ├── include/
│   │   │   └── ramfs.hpp            # 📐 ramfs 接口
│   │   └── ramfs.cpp                # ramfs 实现
│   └── fat32/                       # FAT32 文件系统
│       ├── CMakeLists.txt
│       ├── include/
│       │   └── fat32.hpp            # 📐 FAT32 接口
│       └── fat32.cpp                # FAT32 实现
├── driver/
│   ├── virtio/                      # 📁 新增：virtio 驱动
│   │   ├── CMakeLists.txt
│   │   ├── include/
│   │   │   ├── virtio.hpp           # 📐 virtio 通用接口
│   │   │   └── virtio_blk.hpp       # 📐 virtio-blk 接口
│   │   ├── virtio.cpp               # virtio MMIO transport 实现
│   │   └── virtio_blk.cpp           # virtio-blk 驱动实现
tests/
├── unit_test/
│   ├── vfs_test.cpp                 # VFS 单元测试
│   ├── ramfs_test.cpp               # ramfs 单元测试
│   └── fat32_test.cpp               # FAT32 单元测试（用 mock 块设备）
├── system_test/
│   ├── fs_test.cpp                  # 文件系统集成测试（QEMU 内运行）
│   └── virtio_blk_test.cpp          # virtio-blk 驱动系统测试
```

---

## 3. 接口设计

### 3.1 块设备接口 (`block_device.hpp`)

```cpp
/**
 * @brief 块设备抽象基类
 * @details 所有块设备驱动（virtio-blk、ramdisk 等）必须实现此接口。
 *          块设备以固定大小的扇区 (sector) 为最小 I/O 单位。
 */
class BlockDevice {
 public:
  virtual ~BlockDevice() = default;

  /**
   * @brief 读取连续扇区
   * @param sector_start 起始扇区号（LBA）
   * @param sector_count 扇区数量
   * @param buffer 输出缓冲区，大小至少为 sector_count * GetSectorSize()
   * @return Expected<size_t> 成功时返回实际读取的字节数
   * @pre buffer != nullptr
   * @pre sector_start + sector_count <= GetSectorCount()
   * @post 返回值 == sector_count * GetSectorSize() 或错误
   */
  virtual auto ReadSectors(uint64_t sector_start, uint32_t sector_count,
                           void* buffer) -> Expected<size_t> = 0;

  /**
   * @brief 写入连续扇区
   * @param sector_start 起始扇区号（LBA）
   * @param sector_count 扇区数量
   * @param buffer 输入缓冲区
   * @return Expected<size_t> 成功时返回实际写入的字节数
   * @pre buffer != nullptr
   * @pre sector_start + sector_count <= GetSectorCount()
   */
  virtual auto WriteSectors(uint64_t sector_start, uint32_t sector_count,
                            const void* buffer) -> Expected<size_t> = 0;

  /**
   * @brief 获取扇区大小（通常为 512 字节）
   */
  [[nodiscard]] virtual auto GetSectorSize() const -> uint32_t = 0;

  /**
   * @brief 获取设备总扇区数
   */
  [[nodiscard]] virtual auto GetSectorCount() const -> uint64_t = 0;

  /**
   * @brief 获取设备名称（如 "virtio-blk0"）
   */
  [[nodiscard]] virtual auto GetName() const -> const char* = 0;
};
```

### 3.2 VFS 核心数据结构 (`vfs.hpp`)

```cpp
/// 文件类型
enum class FileType : uint8_t {
  kRegular = 1,    ///< 普通文件
  kDirectory = 2,  ///< 目录
  kCharDevice = 3, ///< 字符设备
  kBlockDevice = 4,///< 块设备
  kSymlink = 5,    ///< 符号链接
};

/// 文件打开标志（兼容 Linux O_* 定义）
enum OpenFlags : uint32_t {
  kOReadOnly  = 0x0000,
  kOWriteOnly = 0x0001,
  kOReadWrite = 0x0002,
  kOCreate    = 0x0040,
  kOTruncate  = 0x0200,
  kOAppend    = 0x0400,
};

/// 文件 seek 基准
enum class SeekWhence : int {
  kSet = 0,  ///< 从文件开头
  kCur = 1,  ///< 从当前位置
  kEnd = 2,  ///< 从文件末尾
};

/**
 * @brief Inode — 文件元数据（独立于路径名）
 * @details 每个文件/目录在 VFS 中有且仅有一个 Inode。
 *          Inode 持有文件的元信息和操作方法指针。
 */
struct Inode {
  uint64_t ino;           ///< inode 编号
  FileType type;          ///< 文件类型
  uint64_t size;          ///< 文件大小（字节）
  uint32_t permissions;   ///< 权限位（简化版）
  uint32_t link_count;    ///< 硬链接计数
  void* fs_private;       ///< 文件系统私有数据指针

  /// inode 操作接口（由具体文件系统填充）
  struct InodeOps {
    auto (*lookup)(Inode* dir, const char* name) -> Expected<Inode*>;
    auto (*create)(Inode* dir, const char* name, FileType type) -> Expected<Inode*>;
    auto (*unlink)(Inode* dir, const char* name) -> Expected<void>;
    auto (*mkdir)(Inode* dir, const char* name) -> Expected<Inode*>;
    auto (*rmdir)(Inode* dir, const char* name) -> Expected<void>;
  } *ops = nullptr;
};

/**
 * @brief Dentry — 目录项缓存（路径名 ↔ Inode 的映射）
 * @details Dentry 构成一棵树，反映目录层次结构。
 *          支持路径查找加速。
 */
struct Dentry {
  char name[256];          ///< 文件/目录名
  Inode* inode;            ///< 关联的 inode
  Dentry* parent;          ///< 父目录项
  Dentry* children;        ///< 子目录项链表头
  Dentry* next_sibling;    ///< 兄弟目录项（同一父目录下）
  void* fs_private;        ///< 文件系统私有数据
};

/**
 * @brief File — 打开的文件实例（每次 open 产生一个）
 * @details File 对象持有当前偏移量和操作方法指针。
 *          多个 File 可以指向同一个 Inode。
 */
struct File {
  Inode* inode;           ///< 关联的 inode
  Dentry* dentry;         ///< 关联的 dentry
  uint64_t offset;        ///< 当前读写偏移量
  uint32_t flags;         ///< 打开标志 (OpenFlags)

  /// 文件操作接口（由具体文件系统填充）
  struct FileOps {
    auto (*read)(File* file, void* buf, size_t count) -> Expected<size_t>;
    auto (*write)(File* file, const void* buf, size_t count) -> Expected<size_t>;
    auto (*seek)(File* file, int64_t offset, SeekWhence whence) -> Expected<uint64_t>;
    auto (*close)(File* file) -> Expected<void>;
    auto (*readdir)(File* file, void* dirent, size_t count) -> Expected<size_t>;
  } *ops = nullptr;
};
```

### 3.3 文件系统基类 (`filesystem.hpp`)

```cpp
/**
 * @brief 文件系统类型基类
 * @details 每种文件系统（ramfs/fat32/ext2 等）注册一个 FileSystem 实例。
 *          VFS 通过此接口挂载/卸载文件系统。
 */
class FileSystem {
 public:
  virtual ~FileSystem() = default;

  /**
   * @brief 获取文件系统类型名（如 "ramfs", "fat32"）
   */
  [[nodiscard]] virtual auto GetName() const -> const char* = 0;

  /**
   * @brief 挂载文件系统
   * @param device 块设备指针（ramfs 等内存文件系统传 nullptr）
   * @return Expected<Inode*> 根目录 inode
   * @post 返回的 inode->type == FileType::kDirectory
   */
  virtual auto Mount(BlockDevice* device) -> Expected<Inode*> = 0;

  /**
   * @brief 卸载文件系统
   * @return Expected<void> 成功或错误
   * @pre 没有打开的文件引用此文件系统
   */
  virtual auto Unmount() -> Expected<void> = 0;

  /**
   * @brief 将缓存数据刷写到磁盘
   * @return Expected<void> 成功或错误
   */
  virtual auto Sync() -> Expected<void> = 0;
};
```

### 3.4 挂载管理 (`mount.hpp`)

```cpp
/**
 * @brief 挂载点
 * @details 将一个文件系统的根 inode 关联到目录树中的某个 dentry 上
 */
struct MountPoint {
  const char* mount_path;     ///< 挂载路径（如 "/mnt/disk"）
  Dentry* mount_dentry;       ///< 挂载点在父文件系统中的 dentry
  FileSystem* filesystem;     ///< 挂载的文件系统实例
  BlockDevice* device;        ///< 关联的块设备（可为 nullptr）
  Inode* root_inode;          ///< 该文件系统的根 inode
  Dentry* root_dentry;        ///< 该文件系统的根 dentry
};

/**
 * @brief 挂载表管理器
 */
class MountTable {
 public:
  /// 最大挂载点数
  static constexpr size_t kMaxMounts = 16;

  /**
   * @brief 挂载文件系统到指定路径
   * @param path 挂载点路径
   * @param fs 文件系统实例
   * @param device 块设备（可为 nullptr）
   * @return Expected<void>
   * @pre path 必须是已存在的目录
   * @post 后续对 path 下路径的访问将被重定向到新文件系统
   */
  auto Mount(const char* path, FileSystem* fs, BlockDevice* device)
      -> Expected<void>;

  /**
   * @brief 卸载指定路径的文件系统
   * @param path 挂载点路径
   * @return Expected<void>
   */
  auto Unmount(const char* path) -> Expected<void>;

  /**
   * @brief 根据路径查找对应的挂载点
   * @param path 文件路径
   * @return 最长前缀匹配的挂载点
   */
  auto Lookup(const char* path) -> MountPoint*;
};
```

### 3.5 文件描述符表 (`file_descriptor.hpp`)

```cpp
/**
 * @brief 进程级文件描述符表
 * @details 每个进程（TaskControlBlock）持有一个 FdTable，
 *          将整数 fd 映射到 File 对象。
 *          fd 0/1/2 预留给 stdin/stdout/stderr。
 */
class FdTable {
 public:
  /// 最大文件描述符数
  static constexpr int kMaxFd = 64;

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
};
```

### 3.6 virtio-blk 驱动 (`virtio.hpp` / `virtio_blk.hpp`)

```cpp
// ===== virtio.hpp: VirtIO MMIO Transport =====

/// VirtIO 设备状态位
enum class VirtIOStatus : uint32_t {
  kAcknowledge = 1,
  kDriver = 2,
  kDriverOk = 4,
  kFeaturesOk = 8,
  kDeviceNeedsReset = 64,
  kFailed = 128,
};

/// VirtQueue 描述符标志
enum class VirtQueueDescFlag : uint16_t {
  kNext = 1,     ///< 描述符链中还有下一个
  kWrite = 2,    ///< 设备可写（设备 → 驱动）
  kIndirect = 4, ///< 间接描述符
};

/**
 * @brief VirtQueue 描述符
 */
struct VirtQueueDesc {
  uint64_t addr;    ///< 缓冲区物理地址
  uint32_t len;     ///< 缓冲区长度
  uint16_t flags;   ///< VirtQueueDescFlag
  uint16_t next;    ///< 下一个描述符索引（如果 flags & kNext）
};

/**
 * @brief VirtIO MMIO Transport
 * @details 管理一个 VirtIO MMIO 设备的寄存器访问和 VirtQueue。
 *          符合 VirtIO 1.0+ MMIO 规范。
 *
 * @note QEMU virt 平台的 virtio MMIO 基地址可通过 FDT 获取。
 *       寄存器布局：偏移 0x000 - 0x0FF。
 */
class VirtIODevice {
 public:
  /**
   * @brief 初始化 VirtIO 设备
   * @param base_addr MMIO 基地址
   * @return Expected<void>
   * @pre base_addr 已经过 VirtualMemory::MapMMIO 映射
   * @post 设备状态为 DRIVER_OK
   */
  auto Init(uint64_t base_addr) -> Expected<void>;

  /**
   * @brief 获取设备 ID（1=net, 2=blk, ...）
   */
  [[nodiscard]] auto GetDeviceId() const -> uint32_t;

  // ... VirtQueue 管理方法（分配、通知、回收）
};

// ===== virtio_blk.hpp: VirtIO Block Device =====

/**
 * @brief VirtIO 块设备驱动
 * @details 基于 VirtIO MMIO transport，实现 BlockDevice 接口。
 *          支持 QEMU 的 virtio-blk-device。
 *
 * @note 设备发现流程：
 *       1. FDT 解析 → 找到 compatible="virtio,mmio" 节点
 *       2. 读取 reg 属性获取 MMIO 基地址
 *       3. MapMMIO 映射
 *       4. 检查 device_id == 2 (block device)
 *       5. 初始化 VirtQueue 并协商特性
 */
class VirtIOBlk : public BlockDevice {
 public:
  /**
   * @brief 初始化 virtio-blk 设备
   * @param base_addr MMIO 基地址（从 FDT 获取）
   * @param irq 中断号（从 FDT 获取）
   * @return Expected<void>
   */
  auto Init(uint64_t base_addr, uint32_t irq) -> Expected<void>;

  // BlockDevice 接口实现
  auto ReadSectors(...) -> Expected<size_t> override;
  auto WriteSectors(...) -> Expected<size_t> override;
  [[nodiscard]] auto GetSectorSize() const -> uint32_t override;
  [[nodiscard]] auto GetSectorCount() const -> uint64_t override;
  [[nodiscard]] auto GetName() const -> const char* override;
};
```

---

## 4. 系统调用接口

在 `syscall.hpp` 中新增以下系统调用号和函数声明：

```cpp
// ===== 新增系统调用号 =====
#if defined(__riscv) || defined(__aarch64__)
static constexpr uint64_t SYSCALL_OPENAT  = 56;
static constexpr uint64_t SYSCALL_CLOSE   = 57;
static constexpr uint64_t SYSCALL_READ    = 63;
// SYSCALL_WRITE = 64 已存在
static constexpr uint64_t SYSCALL_LSEEK   = 62;
static constexpr uint64_t SYSCALL_MOUNT   = 40;
static constexpr uint64_t SYSCALL_UMOUNT  = 39;
#elif defined(__x86_64__)
static constexpr uint64_t SYSCALL_OPEN    = 2;
static constexpr uint64_t SYSCALL_CLOSE   = 3;
static constexpr uint64_t SYSCALL_READ    = 0;
// SYSCALL_WRITE = 1 已存在
static constexpr uint64_t SYSCALL_LSEEK   = 8;
static constexpr uint64_t SYSCALL_MOUNT   = 165;
static constexpr uint64_t SYSCALL_UMOUNT  = 166;
#endif

// ===== 系统调用函数 =====

/**
 * @brief 打开文件
 * @param path 文件路径
 * @param flags 打开标志 (OpenFlags)
 * @param mode 创建模式（权限位）
 * @return 成功返回文件描述符 fd >= 0，失败返回负数错误码
 * @pre path != nullptr
 * @post 成功时在当前进程的 FdTable 中分配一个 fd
 */
int sys_open(const char* path, uint32_t flags, uint32_t mode);

/**
 * @brief 关闭文件描述符
 * @param fd 要关闭的文件描述符
 * @return 0 成功，负数失败
 * @pre fd 是有效的已打开文件描述符
 * @post fd 被释放，关联的 File 对象引用计数减一
 */
int sys_close(int fd);

/**
 * @brief 从文件描述符读取数据
 * @param fd 文件描述符
 * @param buf 输出缓冲区
 * @param count 最大读取字节数
 * @return 成功返回实际读取的字节数（0 表示 EOF），失败返回负数
 * @pre buf != nullptr, count > 0
 * @post file->offset 增加实际读取字节数
 */
int sys_read(int fd, void* buf, size_t count);

// sys_write 已存在，需要扩展以支持通过 VFS 写文件

/**
 * @brief 调整文件偏移量
 * @param fd 文件描述符
 * @param offset 偏移量
 * @param whence 基准 (SEEK_SET/SEEK_CUR/SEEK_END)
 * @return 成功返回新偏移量，失败返回负数
 */
int sys_lseek(int fd, int64_t offset, int whence);

/**
 * @brief 挂载文件系统
 * @param source 块设备路径（如 "/dev/vda"）或 "none"
 * @param target 挂载点路径
 * @param fstype 文件系统类型（"ramfs", "fat32"）
 * @return 0 成功，负数失败
 */
int sys_mount(const char* source, const char* target, const char* fstype);

/**
 * @brief 卸载文件系统
 * @param target 挂载点路径
 * @return 0 成功，负数失败
 */
int sys_umount(const char* target);
```

---

## 5. 改动范围

### 5.1 新增文件

| 文件 | 说明 |
|------|------|
| `src/fs/CMakeLists.txt` | 文件系统子系统构建脚本 |
| `src/fs/include/block_device.hpp` | 块设备抽象接口 |
| `src/fs/include/vfs.hpp` | VFS 核心数据结构 |
| `src/fs/include/filesystem.hpp` | 文件系统基类 |
| `src/fs/include/file_descriptor.hpp` | 文件描述符表 |
| `src/fs/include/mount.hpp` | 挂载管理 |
| `src/fs/vfs.cpp` | VFS 路径解析、inode 缓存等实现 |
| `src/fs/file_descriptor.cpp` | FdTable 实现 |
| `src/fs/mount.cpp` | MountTable 实现 |
| `src/fs/ramfs/CMakeLists.txt` | ramfs 构建脚本 |
| `src/fs/ramfs/include/ramfs.hpp` | ramfs 接口 |
| `src/fs/ramfs/ramfs.cpp` | ramfs 实现 |
| `src/fs/fat32/CMakeLists.txt` | FAT32 构建脚本 |
| `src/fs/fat32/include/fat32.hpp` | FAT32 接口 |
| `src/fs/fat32/fat32.cpp` | FAT32 实现 |
| `src/driver/virtio/CMakeLists.txt` | virtio 驱动构建脚本 |
| `src/driver/virtio/include/virtio.hpp` | virtio MMIO transport 接口 |
| `src/driver/virtio/include/virtio_blk.hpp` | virtio-blk 接口 |
| `src/driver/virtio/virtio.cpp` | virtio MMIO transport 实现 |
| `src/driver/virtio/virtio_blk.cpp` | virtio-blk 驱动实现 |
| `tests/unit_test/vfs_test.cpp` | VFS 单元测试 |
| `tests/unit_test/ramfs_test.cpp` | ramfs 单元测试 |
| `tests/unit_test/fat32_test.cpp` | FAT32 单元测试 |
| `tests/system_test/fs_test.cpp` | 文件系统系统测试 |
| `tests/system_test/virtio_blk_test.cpp` | virtio-blk 系统测试 |

### 5.2 需修改的已有文件

| 文件 | 修改内容 |
|------|---------|
| `src/CMakeLists.txt` | 添加 `ADD_SUBDIRECTORY(fs)`，链接 `fs` 库 |
| `src/driver/CMakeLists.txt` | 添加 `virtio` 子目录（三架构通用） |
| `src/include/syscall.hpp` | 新增文件系统相关系统调用号和函数声明 |
| `src/syscall.cpp` | 新增系统调用分发和实现（open/close/read/write/lseek/mount/umount） |
| `src/include/expected.hpp` | 新增文件系统相关错误码（0x800-0x8FF, 0x900-0x9FF） |
| `src/task/include/task_control_block.hpp` | 添加 `FdTable* fd_table` 字段 |
| `src/main.cpp` | 初始化 VFS、挂载 rootfs (ramfs)、初始化 virtio 设备 |
| `cmake/functions.cmake` | QEMU 启动参数添加 virtio-blk 设备 |
| `src/include/basic_info.hpp` | （可选）添加块设备信息字段 |
| `tests/unit_test/CMakeLists.txt` | 添加新测试文件 |
| `tests/system_test/CMakeLists.txt` | 添加新测试文件 |

### 5.3 QEMU 配置变更

需要为每个架构添加 virtio-blk 设备参数：

```cmake
# 通用：创建磁盘镜像
ADD_CUSTOM_COMMAND(
  OUTPUT ${CMAKE_BINARY_DIR}/bin/disk.img
  COMMAND dd if=/dev/zero of=${CMAKE_BINARY_DIR}/bin/disk.img bs=1M count=64
  COMMAND mkfs.fat -F 32 ${CMAKE_BINARY_DIR}/bin/disk.img
  COMMENT "Creating FAT32 disk image"
)

# QEMU 参数追加（三架构通用）
LIST(APPEND ${PROJECT_NAME}_QEMU_BOOT_FLAGS
  -drive file=${CMAKE_BINARY_DIR}/bin/disk.img,if=none,format=raw,id=hd0
  -device virtio-blk-device,drive=hd0
)
```

### 5.4 新增错误码

```cpp
// 文件系统相关错误 (0x800 - 0x8FF)
kFsFileNotFound = 0x800,
kFsPermissionDenied = 0x801,
kFsNotADirectory = 0x802,
kFsIsADirectory = 0x803,
kFsFileExists = 0x804,
kFsNoSpace = 0x805,
kFsMountFailed = 0x806,
kFsUnmountFailed = 0x807,
kFsInvalidPath = 0x808,
kFsFdTableFull = 0x809,
kFsInvalidFd = 0x80A,
kFsNotMounted = 0x80B,
kFsReadOnly = 0x80C,
kFsCorrupted = 0x80D,

// 块设备相关错误 (0x900 - 0x9FF)
kBlkDeviceNotFound = 0x900,
kBlkReadFailed = 0x901,
kBlkWriteFailed = 0x902,
kBlkSectorOutOfRange = 0x903,

// VirtIO 相关错误 (0xA00 - 0xAFF)
kVirtIOInitFailed = 0xA00,
kVirtIONotBlock = 0xA01,
kVirtIOQueueFull = 0xA02,
kVirtIODeviceError = 0xA03,
kVirtIOFeatureNegotiationFailed = 0xA04,
```

---

## 6. 详细实现计划

### P0: 块设备接口 + virtio-blk 驱动

**目标**：能在 QEMU 中通过 virtio-blk 读写磁盘扇区。

#### 步骤

1. **定义 `BlockDevice` 抽象接口** (`src/fs/include/block_device.hpp`)
2. **实现 VirtIO MMIO transport** (`src/driver/virtio/virtio.cpp`)
   - MMIO 寄存器读写（Magic, Version, DeviceID, Status 等）
   - VirtQueue 分配和初始化（描述符表、Available Ring、Used Ring）
   - 设备初始化握手流程（Reset → Acknowledge → Driver → Features → DriverOk）
   - VirtQueue 通知（写 QueueNotify 寄存器）和回收（轮询/中断）
3. **实现 VirtIO-Blk 驱动** (`src/driver/virtio/virtio_blk.cpp`)
   - 继承 `BlockDevice`
   - 读取设备配置（容量、扇区大小）
   - 构造 `VirtIOBlkReq`（type + sector + data + status）
   - 实现 `ReadSectors` / `WriteSectors`
4. **设备发现**
   - 通过 `KernelFdt` 解析 FDT，查找 `compatible = "virtio,mmio"` 节点
   - 读取 `reg` 属性获取 MMIO 基地址，`interrupts` 属性获取中断号
   - 调用 `VirtualMemory::MapMMIO` 映射设备地址空间
5. **QEMU 配置**
   - 修改 CMake 添加磁盘镜像生成和 QEMU virtio-blk 参数
6. **系统测试**
   - 在 QEMU 中读取已知内容的扇区，验证数据正确性

#### 关键技术点

- VirtIO MMIO 寄存器偏移参考 [VirtIO 1.2 规范 §4.2.2](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)
- VirtQueue 内存需要物理地址连续，使用 `aligned_alloc` 分配
- 当前可使用**轮询模式**（polling），后续可扩展为中断驱动

---

### P1: VFS 框架

**目标**：建立 Inode / Dentry / File 抽象层和路径解析机制。

#### 步骤

1. **定义 VFS 核心结构体** (`src/fs/include/vfs.hpp`)
   - `Inode`、`Dentry`、`File`、`InodeOps`、`FileOps`
2. **实现路径解析** (`src/fs/vfs.cpp`)
   - `VfsLookup(const char* path) -> Expected<Dentry*>`
   - 将 `/a/b/c` 拆分为各级组件，逐级查找
   - 遇到挂载点时切换到子文件系统的根 Dentry
3. **实现 Dentry 树管理**
   - `DentryCreate` / `DentryLookupChild` / `DentryInsertChild`
4. **实现全局 VFS 操作**
   - `VfsOpen(path, flags) -> Expected<File*>`
   - `VfsClose(File*) -> Expected<void>`
   - `VfsRead(File*, buf, count) -> Expected<size_t>`
   - `VfsWrite(File*, buf, count) -> Expected<size_t>`
   - `VfsMount(path, FileSystem*, BlockDevice*) -> Expected<void>`
5. **Inode 缓存**（简化版：直接在 Dentry 中保存，不做额外哈希表缓存）

---

### P2: ramfs

**目标**：纯内存文件系统，作为 rootfs 挂载，验证 VFS 正确性。

#### 步骤

1. **定义 ramfs 接口** (`src/fs/ramfs/include/ramfs.hpp`)
   - `class RamFs : public FileSystem`
2. **实现 InodeOps**
   - `lookup`：在内存中的子节点列表查找
   - `create`：分配新 inode，初始化数据缓冲区
   - `mkdir`：分配新目录 inode
   - `unlink` / `rmdir`：释放 inode 及数据
3. **实现 FileOps**
   - `read`：从内存缓冲区复制数据
   - `write`：扩展内存缓冲区并写入
   - `readdir`：遍历目录子项
4. **数据存储**
   - 使用动态数组（`sk_vector`）或链表（`sk_list`）存储文件内容
   - 目录用子节点链表
5. **测试**
   - 单元测试：创建/删除文件、读写验证、目录操作
   - 系统测试：在 QEMU 中挂载 ramfs 到 `/`，创建文件并读回

---

### P3: FAT32

**目标**：能读写 QEMU virtio-blk 上的 FAT32 文件系统。

#### 步骤

1. **定义 FAT32 磁盘数据结构** (`src/fs/fat32/include/fat32.hpp`)
   - BPB（BIOS Parameter Block）
   - FSInfo 扇区
   - FAT 表项
   - 目录项（短名 + 长名）
2. **实现 `Fat32Fs : public FileSystem`**
   - `Mount`：读取 BPB、FAT 表，构建根目录 inode
   - `Unmount`：刷写脏数据
   - `Sync`：刷写 FAT 表和脏目录项
3. **实现 InodeOps**
   - `lookup`：读取目录簇链，解析目录项
   - `create`：分配空闲簇，写入目录项
   - `unlink`：标记目录项删除，释放 FAT 链
4. **实现 FileOps**
   - `read`：沿 FAT 链读取数据簇
   - `write`：分配新簇、扩展 FAT 链、写入数据
   - `seek`：计算目标簇和偏移
5. **FAT 表管理**
   - 读取 FAT 表到内存缓存
   - 分配/释放簇（首次适配算法）
   - 脏标记 + 定期写回
6. **长文件名支持**（可选，P3 后期）
   - 解析 LFN 目录项序列
   - 拼接成完整文件名
7. **测试**
   - 单元测试：使用 mock BlockDevice + 预构造的 FAT32 镜像
   - 系统测试：在 QEMU 中挂载 virtio-blk 上的 FAT32，读写文件

---

### P4: 系统调用集成

**目标**：用户态可通过系统调用使用文件系统。

#### 步骤

1. **为 `TaskControlBlock` 添加 `FdTable`**
   - 初始化时预设 fd 0/1/2 指向控制台
2. **实现文件系统相关系统调用**
   - `sys_open` → `VfsOpen` → `FdTable::Alloc`
   - `sys_close` → `FdTable::Get` → `VfsClose` → `FdTable::Free`
   - `sys_read` → `FdTable::Get` → `VfsRead`
   - `sys_write` 扩展：fd == 1/2 走原逻辑，其他走 `VfsWrite`
   - `sys_lseek` → `File::ops->seek`
   - `sys_mount` / `sys_umount` → `MountTable`
3. **注册到 `syscall_dispatcher`**
4. **测试**
   - 系统测试：通过 `ecall/syscall` 指令触发文件操作

---

## 7. 关键设计决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| VirtIO transport | MMIO（非 PCI） | QEMU virt 平台默认使用 MMIO；三架构统一 |
| I/O 模式 | 轮询优先，后续加中断 | 简化初始实现，降低调试难度 |
| 磁盘缓存 | 简单扇区缓存 | 先不做 page cache，降低复杂度 |
| ramfs 内存管理 | `aligned_alloc` + `sk_vector` | 复用项目已有的内存分配基础设施 |
| Dentry 缓存 | 链表 | 文件数量有限的教学内核，避免过度设计 |
| FAT 表缓存 | 全量加载到内存 | 64MB 磁盘的 FAT 表约 128KB，完全可控 |
| 文件描述符上限 | 64 | 教学内核够用，避免复杂数据结构 |
| 长文件名 | 延后支持 | 短名（8.3）足以验证核心功能 |

---

## 8. 测试策略

### 单元测试（Host 运行）

| 测试文件 | 覆盖内容 |
|---------|---------|
| `vfs_test.cpp` | 路径解析、Dentry 树操作、FdTable 分配/释放 |
| `ramfs_test.cpp` | 文件创建/删除、读写、目录遍历、边界条件 |
| `fat32_test.cpp` | BPB 解析、FAT 链遍历、目录项解析、簇分配（mock 块设备） |

### 系统测试（QEMU 运行）

| 测试文件 | 覆盖内容 |
|---------|---------|
| `virtio_blk_test.cpp` | 设备发现、扇区读写、边界扇区 |
| `fs_test.cpp` | ramfs 挂载 + 文件操作、FAT32 挂载 + 跨簇读写、mount/umount |

---

## 9. 里程碑与工作量估算

| 里程碑 | 预估工作 | 验收标准 |
|--------|---------|---------|
| M0: BlockDevice + virtio-blk | ~800 行代码 | QEMU 中能读写磁盘扇区并打印内容 |
| M1: VFS 框架 | ~600 行代码 | 路径解析、Dentry 树、File 操作流程可走通 |
| M2: ramfs | ~400 行代码 | 能挂载 ramfs 到 `/`，创建文件、写入、读回 |
| M3: FAT32 | ~1000 行代码 | 能挂载 virtio-blk 上的 FAT32，读写文件 |
| M4: 系统调用 | ~300 行代码 | 用户态通过 syscall 完成 open/read/write/close |
| **总计** | **~3100 行代码** | 完整文件系统栈可用 |

---

## 10. 参考资料

- [VirtIO 1.2 规范](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html) — §2 (基本设施), §4.2 (MMIO), §5.2 (Block Device)
- [Microsoft FAT32 规范](https://academy.cba.mit.edu/classes/networking_communications/SD/FAT.pdf) — BPB, FAT, Directory Entry
- [Linux VFS 文档](https://www.kernel.org/doc/html/latest/filesystems/vfs.html) — 设计参考（不需要完全复制）
- [xv6-riscv](https://github.com/mit-pdos/xv6-riscv) — 简洁文件系统实现参考
- [OSDev Wiki: VirtIO](https://wiki.osdev.org/Virtio) — 社区驱动开发指南
