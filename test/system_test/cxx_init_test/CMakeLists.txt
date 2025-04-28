# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

# 设置项目名与版本
PROJECT (cxx-init-test VERSION 0.0.1)

ENABLE_LANGUAGE (CXX)
ENABLE_LANGUAGE (ASM)

ADD_EXECUTABLE (
    ${PROJECT_NAME}
    main.cpp
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:
    ${CMAKE_SOURCE_DIR}/src/arch/${CMAKE_SYSTEM_PROCESSOR}/boot.S
    >
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:
    ${CMAKE_SOURCE_DIR}/src/arch/${CMAKE_SYSTEM_PROCESSOR}/boot.S
    >)

TARGET_INCLUDE_DIRECTORIES (
    ${PROJECT_NAME}
    PRIVATE ${CMAKE_SOURCE_DIR}/src/include ${CMAKE_SOURCE_DIR}/src/arch
            ${CMAKE_SOURCE_DIR}/src/arch/${CMAKE_SYSTEM_PROCESSOR}/include)

TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE kernel_link_libraries libc
                                               libcxx)

# 获取目标文件信息
OBJDUMP_READELF_NM (${PROJECT_NAME})
