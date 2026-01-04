/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SINGLETON_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SINGLETON_HPP_

// 单例模板类
template <typename T>
class Singleton {
 public:
  /// @name 构造/析构函数
  /// @{
  Singleton() = default;
  Singleton(const Singleton&) = delete;
  Singleton(Singleton&&) = delete;
  auto operator=(const Singleton&) -> Singleton& = delete;
  auto operator=(Singleton&&) -> Singleton& = delete;
  ~Singleton() = default;
  /// @}

  // 获取单例实例的静态方法
  static auto GetInstance() -> T& {
    static T instance;
    return instance;
  }
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SINGLETON_HPP_ */
