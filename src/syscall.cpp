/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "syscall.hpp"

#include "kernel_log.hpp"
#include "singleton.hpp"

int syscall_dispatcher(int64_t syscall_id, uint64_t args[6]) {
  int64_t ret = 0;
  switch (syscall_id) {
    case SYSCALL_WRITE: {
      ret = sys_write(static_cast<int>(args[0]),
                      reinterpret_cast<const char*>(args[1]),
                      static_cast<size_t>(args[2]));
      break;
    }
    default: {
      klog::Err("[Syscall] Unknown syscall id: %d\n", syscall_id);
      ret = -1;
      break;
    }
  }
  return ret;
}

int sys_write(int fd, const char* buf, size_t len) {
  // 简单实现：仅支持向标准输出(1)和错误输出(2)打印
  if (fd == 1 || fd == 2) {
    // TODO: 应该检查 buf 是否在用户空间合法范围内
    for (size_t i = 0; i < len; ++i) {
      // 使用 klog::Print 而不是 Format，避免 buffer 解析带来的问题
      sk_printf("%c", buf[i]);
    }
    return static_cast<int>(len);
  }
  return -1;
}
