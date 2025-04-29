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
TARGET_INCLUDE_DIRECTORIES (${PROJECT_NAME} PRIVATE include)

# 添加要链接的库
TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE kernel_link_libraries libc
                                               libcxx arch driver)

SET_TARGET_PROPERTIES (${PROJECT_NAME} PROPERTIES OUTPUT_NAME
                                                  ${KERNEL_ELF_OUTPUT_NAME})

# 获取目标文件信息
OBJDUMP_READELF_NM (${PROJECT_NAME})

# 生成 QEMU DTS 和 DTB
ADD_CUSTOM_COMMAND (
    COMMENT "Generating QEMU DTS and DTB ..."
    VERBATIM
    OUTPUT ${CMAKE_BINARY_DIR}/bin/qemu.dtb ${CMAKE_BINARY_DIR}/bin/qemu.dts
    COMMAND
        qemu-system-${CMAKE_SYSTEM_PROCESSOR} ${QEMU_COMMON_FLAG}
        ${QEMU_MACHINE_FLAGS} -machine dumpdtb=${CMAKE_BINARY_DIR}/bin/qemu.dtb
    COMMAND dtc -I dtb ${CMAKE_BINARY_DIR}/bin/qemu.dtb -O dts -o
            ${CMAKE_BINARY_DIR}/bin/qemu.dts)

# 生成 U-BOOT FIT
ADD_CUSTOM_COMMAND (
    COMMENT "Generating U-BOOT FIT ..."
    VERBATIM
    DEPENDS
        $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:${CMAKE_BINARY_DIR}/bin/qemu.dtb>
        $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:${CMAKE_BINARY_DIR}/bin/qemu.dtb>
    OUTPUT ${CMAKE_BINARY_DIR}/bin/boot.fit
           ${CMAKE_BINARY_DIR}/bin/boot.scr.uimg
    COMMAND mkimage -f ${CMAKE_BINARY_DIR}/bin/boot.its
            ${CMAKE_BINARY_DIR}/bin/boot.fit
    COMMAND
        mkimage -T script -d
        ${CMAKE_SOURCE_DIR}/tools/${CMAKE_SYSTEM_PROCESSOR}_boot_scr.txt
        ${CMAKE_BINARY_DIR}/bin/boot.scr.uimg)

# 添加 run 和 debug target
ADD_RUN_TARGET (
    DEPENDS
    u-boot
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:opensbi>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:optee_os>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:arm-trusted-firmware>
    ${CMAKE_BINARY_DIR}/bin/boot.fit
    ${CMAKE_BINARY_DIR}/bin/boot.scr.uimg
    ${PROJECT_NAME}
    QEMU_FLAGS
    ${QEMU_COMMON_FLAG}
    ${QEMU_MACHINE_FLAGS}
    ${QEMU_BOOT_FLAGS})
