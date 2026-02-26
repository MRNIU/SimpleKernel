/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_FILESYSTEM_FATFS_INCLUDE_FFCONF_H_
#define SIMPLEKERNEL_SRC_FILESYSTEM_FATFS_INCLUDE_FFCONF_H_

/*---------------------------------------------------------------------------/
/  SimpleKernel FatFs 配置
/---------------------------------------------------------------------------*/

#define FFCONF_DEF 80386 /* 版本号 */

/*---------------------------------------------------------------------------/
/  功能配置
/---------------------------------------------------------------------------*/

/* 只读模式：0=读写，1=只读 */
#define FF_FS_READONLY 0

/* 最小化级别：0=完整，1/2/3=逐级裁剪 API */
#define FF_FS_MINIMIZE 0

/* 目录过滤读取（f_findfirst/f_findnext）：0=禁用，1/2=启用 */
#define FF_USE_FIND 0

/* 格式化（f_mkfs）：0=禁用，1=启用 */
#define FF_USE_MKFS 0

/* 快速定位：0=禁用，1=启用 */
#define FF_USE_FASTSEEK 0

/* 文件扩展（f_expand）：0=禁用，1=启用 */
#define FF_USE_EXPAND 0

/* 属性控制（f_chmod/f_utime）：0=禁用，1=启用 */
#define FF_USE_CHMOD 0

/* 卷标 API（f_getlabel/f_setlabel）：0=禁用，1=启用 */
#define FF_USE_LABEL 0

/* 流转发（f_forward）：0=禁用，1=启用 */
#define FF_USE_FORWARD 0

/* 字符串 I/O（f_gets/f_putc/f_puts/f_printf）：0=禁用，1/2=启用 */
#define FF_USE_STRFUNC 0
#define FF_PRINT_LLI 0
#define FF_PRINT_FLOAT 0
#define FF_STRF_ENCODE 0

/*---------------------------------------------------------------------------/
/  区域与命名空间配置
/---------------------------------------------------------------------------*/

/* OEM 代码页：437=美国英语 */
#define FF_CODE_PAGE 437

/* 长文件名（LFN）：0=禁用，1=静态 BSS，2=栈，3=堆 */
#define FF_USE_LFN 1
#define FF_MAX_LFN 255

/* LFN 字符编码：0=ANSI/OEM，1=UTF-16LE，2=UTF-8，3=UTF-32 */
#define FF_LFN_UNICODE 0

/* FILINFO 成员中文件名缓冲区大小 */
#define FF_LFN_BUF 255
#define FF_SFN_BUF 12

/* 相对路径：0=禁用，1=启用，2=+f_getcwd */
#define FF_FS_RPATH 2

/* exFAT 最大目录深度（仅 exFAT 有效） */
#define FF_PATH_DEPTH 10

/*---------------------------------------------------------------------------/
/  驱动器/卷配置
/---------------------------------------------------------------------------*/

/* 逻辑驱动器数量（1-10） */
#define FF_VOLUMES 2

/* 字符串卷 ID：0=禁用，1/2=启用 */
#define FF_STR_VOLUME_ID 0
#define FF_VOLUME_STRS "RAM", "SD"

/* 多分区支持：0=禁用，1=启用 */
#define FF_MULTI_PARTITION 0

/* 扇区大小范围（字节）：通常均设为 512 */
#define FF_MIN_SS 512
#define FF_MAX_SS 512

/* 64 位 LBA：0=禁用，1=启用 */
#define FF_LBA64 0

/* f_mkfs/f_fdisk 切换 GPT 的最小扇区数 */
#define FF_MIN_GPT 0x10000000

/* ATA-TRIM：0=禁用，1=启用 */
#define FF_USE_TRIM 0

/*---------------------------------------------------------------------------/
/  系统配置
/---------------------------------------------------------------------------*/

/* 精简缓冲区：0=普通，1=精简 */
#define FF_FS_TINY 0

/* exFAT 支持：0=禁用，1=启用 */
#define FF_FS_EXFAT 0

/* 时间戳：FF_FS_NORTC=1 表示无 RTC，使用固定时间 */
#define FF_FS_NORTC 1
#define FF_NORTC_MON 1
#define FF_NORTC_MDAY 1
#define FF_NORTC_YEAR 2025

/* 创建时间记录：0=禁用，1=启用 */
#define FF_FS_CRTIME 0

/* FSINFO 信任策略：bit0=不信任空闲簇数，bit1=不信任最后分配簇号 */
#define FF_FS_NOFSINFO 0

/* 文件锁：0=禁用，>0=最大同时打开对象数 */
#define FF_FS_LOCK 0

/* 重入（线程安全）：0=禁用，1=启用（需提供 ff_mutex_* 实现） */
#define FF_FS_REENTRANT 0
#define FF_FS_TIMEOUT 1000

/*--- End of configuration options ---*/

#endif /* SIMPLEKERNEL_SRC_FILESYSTEM_FATFS_INCLUDE_FFCONF_H_ */
