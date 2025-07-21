# Copyright The SimpleKernel Contributors
# CMake build configuration

# 设置项目名与版本
PROJECT (opensbi-test VERSION 0.0.1)

ENABLE_LANGUAGE (ASM)
ENABLE_LANGUAGE (CXX)

ADD_EXECUTABLE (
    ${PROJECT_NAME}
    main.cpp ${CMAKE_SOURCE_DIR}/src/arch/${CMAKE_SYSTEM_PROCESSOR}/boot.S)

# 添加要链接的库
TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE kernel_link_libraries libc
                                               libcxx)

# 获取目标文件信息
# OBJDUMP_READELF_NM (${PROJECT_NAME})
