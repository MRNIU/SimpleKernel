# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

# 设置项目名与版本
PROJECT (kernel VERSION 0.0.1)

ENABLE_LANGUAGE (ASM)
ENABLE_LANGUAGE (C)
ENABLE_LANGUAGE (CXX)

ADD_SUBDIRECTORY (libc)
ADD_SUBDIRECTORY (libcxx)
ADD_SUBDIRECTORY (arch)
ADD_SUBDIRECTORY (driver)

ADD_EXECUTABLE (${PROJECT_NAME} main.cpp)

# 添加头文件
TARGET_INCLUDE_DIRECTORIES (kernel PRIVATE include)

# 添加要链接的库
TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE kernel_link_libraries libc
                                               libcxx arch driver)

SET_TARGET_PROPERTIES (${PROJECT_NAME} PROPERTIES PREFIX "")
SET_TARGET_PROPERTIES (${PROJECT_NAME} PROPERTIES OUTPUT_NAME
                                                  ${KERNEL_ELF_OUTPUT_NAME})

# 获取目标文件信息
OBJDUMP_READELF_NM (${PROJECT_NAME})
