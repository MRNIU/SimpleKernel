# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

# 设置项目名与版本
PROJECT (gnu-efi-test VERSION 0.0.1)

ENABLE_LANGUAGE (CXX)

ADD_EXECUTABLE (${PROJECT_NAME} main.cpp)

TARGET_INCLUDE_DIRECTORIES (
    ${PROJECT_NAME}
    PRIVATE ${CMAKE_SOURCE_DIR}/src/include ${CMAKE_SOURCE_DIR}/src/arch
            ${CMAKE_SOURCE_DIR}/src/arch/${CMAKE_SYSTEM_PROCESSOR}/include)

TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE kernel_link_libraries libc
                                               libcxx)

SET_TARGET_PROPERTIES (${PROJECT_NAME} PROPERTIES PREFIX "")
SET_TARGET_PROPERTIES (${PROJECT_NAME} PROPERTIES OUTPUT_NAME
                                                  ${KERNEL_ELF_OUTPUT_NAME})

# 获取目标文件信息
OBJDUMP_READELF_NM (${PROJECT_NAME})

# 添加 run 和 debug target
ADD_RUN_TARGET (
    NAME
    ${PROJECT_NAME}_
    DEPENDS
    ${PROJECT_NAME}
    WORKING_DIRECTORY
    ${PROJECT_BINARY_DIR}
    TARGET
    ${CMAKE_SYSTEM_PROCESSOR}
    KERNEL
    $<TARGET_FILE:${PROJECT_NAME}>
    QEMU_FLAGS
    ${QEMU_FLAGS})
