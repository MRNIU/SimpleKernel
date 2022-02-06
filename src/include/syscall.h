
/**
 * @file syscall.h
 * @brief 系统调用头文件
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2022-01-30
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2022-01-30<td>MRNIU<td>创建文件
 * </table>
 */

#ifndef _SYSCALL_H_
#define _SYSCALL_H_

#include "stdint.h"
#include "stddef.h"

/**
 * @brief 字符输出
 * @param _c               要输出的字符
 * @return                 成功返回 0
 */
int32_t sys_putc(char _c);

class SYSCALL {
private:
    /// 最多 256 个系统调用
    static constexpr const uint8_t SYSCALL_NO_MAX = UINT8_MAX;

    /**
     * @brief 系统调用函数指针
     * @param  _argv           参数列表
     * @return int32_t         返回值，0 成功
     */
    typedef int32_t (*syscall_handler_t)(uintptr_t *_argv);

    /// 系统调用函数指针表
    static syscall_handler_t syscalls[SYSCALL_NO_MAX];

    /**
     * @brief 执行系统调用，中断处理程序使用
     * @param _no              系统调用号
     * @param _argv            系统调用参数
     * @return                 成功返回 0
     */
    int32_t do_syscall(uint8_t _no, uintptr_t *_argv);

protected:
public:
    /// 系统调用最大参数数量
    static constexpr const size_t MAX_ARGS = 7;

    /// 系统调用号
    enum : uint8_t {
        SYS_putc = 0,
    };

    /**
     * @brief 获取单例
     * @return SYSCALL&    静态对象
     */
    static SYSCALL &get_instance(void);

    /**
     * @brief 系统调用初始化
     * @return int32_t         成功返回 0
     */
    int32_t init(void);
    int32_t init_other_core(void);

    /**
     * @brief 系统调用入口，构造参数并触发中断
     * @param  _sysno           系统调用号
     * @param  ...              系统调用参数
     * @return int32_t          成功返回 0
     */
    inline int32_t syscall(uint8_t _sysno, ...);

    friend int32_t u_env_call_handler(int _argc, char **_argv);
};

#endif /* _SYSCALL_H_ */
