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

# qemu 参数设置
LIST (APPEND ${PROJECT_NAME}_QEMU_BOOT_FLAGS "")
# 目标平台参数
IF(CMAKE_SYSTEM_PROCESSOR STREQUAL "x86_64")
    LIST (APPEND ${PROJECT_NAME}_QEMU_BOOT_FLAGS -drive
          file=fat:rw:${CMAKE_BINARY_DIR}/bin,format=raw,media=disk -bios
          ${u-boot_BINARY_DIR}/u-boot.rom)
ELSEIF(CMAKE_SYSTEM_PROCESSOR STREQUAL "riscv64")
    LIST (APPEND ${PROJECT_NAME}_QEMU_BOOT_FLAGS -bios
          3rd/opensbi/platform/generic/firmware/fw_jump.elf -kernel
          $<TARGET_FILE:${PROJECT_NAME}>)
ELSEIF(CMAKE_SYSTEM_PROCESSOR STREQUAL "aarch64")
    LIST (APPEND ${PROJECT_NAME}_QEMU_BOOT_FLAGS -kernel
          $<TARGET_FILE:${PROJECT_NAME}>)
ENDIF()

# 添加 run 和 debug targets
ADD_RUN_TARGET (
    NAME
    ${PROJECT_NAME}_
    TARGET
    ${PROJECT_NAME}
    DEPENDS
    u-boot
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:opensbi>
    QEMU_BOOT_FLAGS
    ${${PROJECT_NAME}_QEMU_BOOT_FLAGS})
