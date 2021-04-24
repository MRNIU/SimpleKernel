
// This file is a part of Simple-XX/SimpleKernel
// (https://github.com/Simple-XX/SimpleKernel).
//
// ramfs.h for Simple-XX/SimpleKernel.

#ifndef _RAMFS_H_
#define _RAMFS_H_

#include "vfs.h"

class RAMFS : FS {
private:
    // 开始地址
    void *start;
    // 结束地址
    void *end;
    // 512MB
    static constexpr const uint32_t size = 0x20000000;
    // inode 数量
    static constexpr const uint32_t inode_size = 0x20000000;
    // 超级块
    // inode 索引

protected:
public:
    RAMFS(void);
    ~RAMFS(void);
    int get_sb(void);
    int kill_sb(void);
    int read_super(void);
    int open();
    int close();
    int seek();
};

static RAMFS ramfs;

#endif /* _RAMFS_H_ */
