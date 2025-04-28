# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# project_config.cmake for Simple-XX/SimpleKernel.
# 配置信息

# 在目标环境搜索 program
SET (CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
# 在目标环境搜索库文件
SET (CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
# 在目标环境搜索头文件
SET (CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)

# 目标架构
LIST (APPEND VALID_CMAKE_SYSTEM_PROCESSOR x86_64 riscv64 aarch64)
# 默认构建 x86_64
IF(NOT DEFINED CMAKE_SYSTEM_PROCESSOR)
    SET (CMAKE_SYSTEM_PROCESSOR x86_64)
ENDIF()
MESSAGE (STATUS "CMAKE_SYSTEM_PROCESSOR is: ${CMAKE_SYSTEM_PROCESSOR}")
# 如果不合法则报错
IF(NOT CMAKE_SYSTEM_PROCESSOR IN_LIST VALID_CMAKE_SYSTEM_PROCESSOR)
    MESSAGE (FATAL_ERROR "CMAKE_SYSTEM_PROCESSOR must be one of \
            ${VALID_CMAKE_SYSTEM_PROCESSOR}")
ENDIF()

MESSAGE (STATUS "CMAKE_TOOLCHAIN_FILE is: ${CMAKE_TOOLCHAIN_FILE}")
# 编译器只支持 gnu-gcc 或 clang
IF(NOT ("${CMAKE_CXX_COMPILER_ID}" MATCHES "GNU" OR "${CMAKE_CXX_COMPILER_ID}"
                                                    MATCHES "Clang"))
    MESSAGE (FATAL_ERROR "Only support gnu-gcc/clang")
ENDIF()

# qemu gdb 调试端口
IF(NOT DEFINED QEMU_GDB_PORT)
    SET (QEMU_GDB_PORT tcp::1234)
ENDIF()

# 生成项目配置头文件，传递给代码
CONFIGURE_FILE (
    ${CMAKE_SOURCE_DIR}/tools/${CMAKE_SYSTEM_PROCESSOR}_qemu_virt.its.in
    ${CMAKE_BINARY_DIR}/bin/boot.its @ONLY)
CONFIGURE_FILE (${CMAKE_SOURCE_DIR}/tools/project_config.h.in
                ${CMAKE_SOURCE_DIR}/src/project_config.h @ONLY)
CONFIGURE_FILE (${CMAKE_SOURCE_DIR}/tools/.pre-commit-config.yaml.in
                ${CMAKE_SOURCE_DIR}/.pre-commit-config.yaml @ONLY)
