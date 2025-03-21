/**
 * @file main.cpp
 * @brief 内核入口
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-07-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-07-15<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"
#include "sk_libcxx.h"

/// 用于判断是否为启动核
namespace {

bool is_boot_core = false;
/// 非启动核入口
auto main_smp(int argc, const char **argv) -> int {
  ArchInitSMP(argc, argv);
  klog::Info("Hello SimpleKernel\n");
  return 0;
}

}  // namespace

void _start(int argc, const char **argv) {
  if (!is_boot_core) {
    is_boot_core = true;
    CppInit();
    main(argc, argv);
    CppDeInit();
  } else {
    main_smp(argc, argv);
  }

  // 进入死循环
  while (true) {
    ;
  }
}

auto main(int argc, const char **argv) -> int {
  // 架构相关初始化
  ArchInit(argc, argv);

  // klog::Debug("Hello SimpleKernel\n");
  // klog::Info("Hello SimpleKernel\n");
  // klog::Warn("Hello SimpleKernel\n");
  // klog::Err("Hello SimpleKernel\n");

  DumpStack();

  klog::info << "Hello SimpleKernel\n";

  return 0;
}
