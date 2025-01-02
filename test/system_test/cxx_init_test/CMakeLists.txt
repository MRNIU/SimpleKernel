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
    ${CMAKE_SOURCE_DIR}/src/kernel/arch/${CMAKE_SYSTEM_PROCESSOR}/boot.S
    >
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:
    ${CMAKE_SOURCE_DIR}/src/kernel/arch/${CMAKE_SYSTEM_PROCESSOR}/boot.S
    >)

TARGET_INCLUDE_DIRECTORIES (
    ${PROJECT_NAME}
    PRIVATE
        ${CMAKE_SOURCE_DIR}/src/kernel/include
        ${CMAKE_SOURCE_DIR}/src/kernel/arch
        ${CMAKE_SOURCE_DIR}/src/kernel/arch/${CMAKE_SYSTEM_PROCESSOR}/include)

TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE kernel_link_libraries libc
                                               libcxx)

SET_TARGET_PROPERTIES (${PROJECT_NAME} PROPERTIES PREFIX "")
SET_TARGET_PROPERTIES (${PROJECT_NAME} PROPERTIES OUTPUT_NAME
                                                  ${KERNEL_ELF_OUTPUT_NAME})

# 获取目标文件信息
OBJDUMP_READELF_NM (${PROJECT_NAME})

# 添加 run 和 debug target
IF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "x86_64")
    ADD_RUN_TARGET (
        NAME
        ${PROJECT_NAME}_
        DEPENDS
        boot
        ${PROJECT_NAME}
        WORKING_DIRECTORY
        ${PROJECT_BINARY_DIR}
        TARGET
        ${CMAKE_SYSTEM_PROCESSOR}
        BOOT
        ${boot_BINARY_DIR}/${BOOT_EFI_OUTPUT_NAME}
        KERNEL
        $<TARGET_FILE:${PROJECT_NAME}>
        QEMU_FLAGS
        ${QEMU_FLAGS})
ELSEIF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "riscv64")
    ADD_RUN_TARGET (
        NAME
        ${PROJECT_NAME}_
        DEPENDS
        boot
        ${PROJECT_NAME}
        WORKING_DIRECTORY
        ${PROJECT_BINARY_DIR}
        TARGET
        ${CMAKE_SYSTEM_PROCESSOR}
        BOOT
        ${boot_BINARY_DIR}/${BOOT_EFI_OUTPUT_NAME}
        KERNEL
        $<TARGET_FILE:${PROJECT_NAME}>
        QEMU_FLAGS
        -serial
        stdio
        -monitor
        telnet::2333,server,nowait
        -machine
        virt
        -nographic
        -bios
        ${opensbi_BINARY_DIR}/platform/generic/firmware/fw_jump.elf
        -kernel
        $<TARGET_FILE:${PROJECT_NAME}>)
ELSEIF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
    ADD_RUN_TARGET (
        NAME
        ${PROJECT_NAME}_
        DEPENDS
        boot
        ${PROJECT_NAME}
        WORKING_DIRECTORY
        ${PROJECT_BINARY_DIR}
        TARGET
        ${CMAKE_SYSTEM_PROCESSOR}
        BOOT
        ${boot_BINARY_DIR}/${BOOT_EFI_OUTPUT_NAME}
        KERNEL
        $<TARGET_FILE:${PROJECT_NAME}>
        QEMU_FLAGS
        -serial
        stdio
        -monitor
        telnet::2333,server,nowait
        -machine
        virt
        -nographic
        -cpu
        cortex-a72
        -m
        128M
        -net
        none
        -kernel
        $<TARGET_FILE:${PROJECT_NAME}>)
ENDIF()
