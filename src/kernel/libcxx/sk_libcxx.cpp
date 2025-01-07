
/**
 * @file sk_libcxx.cpp
 * @brief sk_libcxx cpp
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

#include "sk_libcxx.h"

#include <algorithm>

/// 全局构造函数函数指针
using function_t = void (*)();
// 在 link.ld 中定义
/// 全局构造函数函数指针起点地址
extern "C" function_t _init_array_start;
/// 全局构造函数函数指针终点地址
extern "C" function_t _init_array_end;
/// 全局析构函数函数指针起点地址
extern "C" function_t _fini_array_start;
/// 全局析构函数函数指针终点地址
extern "C" function_t _fini_array_end;
/// 动态共享对象标识，内核使用静态链接，此变量在内核中没有使用
void* dso_handle = nullptr;

/// 最大析构函数数量
static constexpr const size_t kMaxAtExitFuncsCount = 128;

/// 析构函数结构
struct atexit_func_entry_t {
  // 析构函数指针
  void (*destructor_func)(void*);
  // 要析构的对象
  void* obj_ptr;
  // 共享库对象，在内核中没有使用
  void* dso_handle;
};

/// 析构函数数组
static atexit_func_entry_t atexit_funcs[kMaxAtExitFuncsCount];  // NOLINT
/// 析构函数个数
static size_t atexit_func_count = 0;

/**
 * 注册在程序正常终止时调用的析构函数
 * @param destructor_func 指向要调用的析构函数的指针
 * @param obj_ptr 传递给析构函数的参数
 * @return 成功返回 0
 */
extern "C" auto _cxa_atexit(void (*destructor_func)(void*), void* obj_ptr,
                            void*) -> int {
  if (atexit_func_count >= kMaxAtExitFuncsCount) {
    return -1;
  }
  atexit_funcs[atexit_func_count].destructor_func = destructor_func;
  atexit_funcs[atexit_func_count].obj_ptr = obj_ptr;
  atexit_funcs[atexit_func_count].dso_handle = nullptr;
  atexit_func_count++;
  return 0;
}

/**
 * 调用析构函数
 * @param destructor_func 要调用的析构函数指针，为 nullptr
 * 时调用所有注册的析构函数。
 */
extern "C" void _cxa_finalize(void* destructor_func) {
  if (destructor_func == nullptr) {
    // 如果 destructor_func 为 nullptr，调用所有析构函数
    for (auto i = atexit_func_count - 1; i != 0; i--) {
      if (atexit_funcs[i].destructor_func != nullptr) {
        (*atexit_funcs[i].destructor_func)(atexit_funcs[i].obj_ptr);
      }
    }
  } else {
    // 不为空时只调用对应的析构函数
    for (auto i = atexit_func_count - 1; i != 0; i--) {
      if ((void*)atexit_funcs[i].destructor_func == destructor_func) {
        (*atexit_funcs[i].destructor_func)(atexit_funcs[i].obj_ptr);
        atexit_funcs[i].destructor_func = nullptr;
      }
    }
  }
}

/// @name 保证静态局部变量线程安全
/// @todo 确保正确
/// @{
/**
 * if ( obj_guard.first_byte == 0 ) {
 *     if ( __cxa_guard_acquire(&obj_guard) ) {
 *       try {
 *         ... initialize the object ...;
 *       }
 *       catch (...) {
 *         __cxa_guard_abort(&obj_guard);
 *         throw;
 *       }
 *       ... queue object destructor with __cxa_atexit() ...;
 *       __cxa_guard_release(&obj_guard);
 *     }
 *  }
 */

struct GuardType {
  /// 是否被使用
  uint64_t is_in_use : 8;
  /// 是否已初始化
  uint64_t is_initialized : 8;
  uint64_t pad : 48;
};

/**
 * 检测静态局部变量是否已经初始化
 * @param guard 锁，一个 64 位变量
 * @return 未初始化返回非零值，已初始化返回 0
 */
extern "C" auto _cxa_guard_acquire(GuardType* guard) -> int {
  if ((guard->is_in_use == 0U) && (guard->is_initialized == 0U)) {
    guard->is_in_use = 1;
  }
  return static_cast<int>(static_cast<int>(guard->is_initialized) == 0U);
}

/**
 * 用于检测静态局部变量是否已经初始化，并设置锁
 * @param guard 锁，一个 64 位变量
 * @return 未初始化返回非零值并设置锁，已初始化返回 0
 */
extern "C" void _cxa_guard_release(GuardType* guard) {
  guard->is_in_use = 0;
  guard->is_initialized = 1;
}

/**
 * 如果在初始化过程中出现异常或其他错误，调用此函数以释放锁而不标记变量为已初始化
 * @param guard 锁
 */
extern "C" void _cxa_guard_abort(GuardType* guard) {
  guard->is_in_use = 0;
  guard->is_initialized = 0;
}

/// @}

/**
 * 纯虚函数调用处理
 */
extern "C" void _cxa_pure_virtual() {
  while (true) {
    ;
  }
}

/**
 * 异常处理
 * @note 这里只能处理 throw，无法处理异常类型
 */
extern "C" void _cxa_rethrow() {
  while (true) {
    ;
  }
}
extern "C" void Unwind_Resume() {
  while (true) {
    ;
  }
}
extern "C" void _gxx_personality_v0() {
  while (true) {
    ;
  }
}

/**
 * c++ 全局对象构造
 */
void CppInit() {
  // 调用构造函数
  std::for_each(&_init_array_start, &_init_array_end,
                [](function_t func) { (func)(); });
}

/**
 * c++ 全局对象析构
 */
void CppDeInit() {
  // 调用析构函数
  std::for_each(&_fini_array_start, &_fini_array_end,
                [](function_t func) { (func)(); });
}
