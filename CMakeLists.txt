# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

# 设置最小 cmake 版本
CMAKE_MINIMUM_REQUIRED (VERSION 3.27 FATAL_ERROR)

# 设置项目名与版本
PROJECT (SimpleKernel VERSION 0.0.1)

# 禁止原地编译
IF(${PROJECT_SOURCE_DIR} STREQUAL ${PROJECT_BINARY_DIR})
    # 如果你看到这句话，cmake 此时已经在根目录下生成了一些临时文件，你需要删除它们 CMakeFiles, CMakeCache.txt
    MESSAGE (
        FATAL_ERROR
            "In-source builds not allowed."
            "Please make a new directory (called a build directory) "
            "and run CMake from there.")
ENDIF()

# 设置辅助 cmake 脚本路径
LIST (APPEND CMAKE_MODULE_PATH "${PROJECT_SOURCE_DIR}/cmake")

# 导入项目配置
INCLUDE (project_config)
# 导入函数
INCLUDE (functions)

# 导入第三方依赖
INCLUDE (3rd)

# 导入编译配置
INCLUDE (compile_config)

# 添加要编译的目录
ADD_SUBDIRECTORY (${PROJECT_SOURCE_DIR}/src)
ADD_SUBDIRECTORY (${PROJECT_SOURCE_DIR}/test)
ADD_SUBDIRECTORY (${PROJECT_SOURCE_DIR}/doc)

# qemu 参数设置
LIST (APPEND QEMU_BOOT_FLAGS "")
LIST (APPEND QEMU_MACHINE_FLAGS "")
# 目标平台参数
IF(CMAKE_SYSTEM_PROCESSOR STREQUAL "x86_64")
    LIST (APPEND QEMU_BOOT_FLAGS -bios ${u-boot_BINARY_DIR}/u-boot.rom)
ELSEIF(CMAKE_SYSTEM_PROCESSOR STREQUAL "riscv64")
    LIST (APPEND QEMU_MACHINE_FLAGS -machine virt)
    LIST (APPEND QEMU_BOOT_FLAGS -bios ${u-boot_BINARY_DIR}/spl/u-boot-spl.bin
          -device loader,file=${u-boot_BINARY_DIR}/u-boot.itb,addr=0x80200000)
ELSEIF(CMAKE_SYSTEM_PROCESSOR STREQUAL "aarch64")
    LIST (APPEND QEMU_MACHINE_FLAGS -machine virt,secure=on,gic_version=3 -cpu
          cortex-a72)
    LIST (
        APPEND
        QEMU_BOOT_FLAGS
        -drive
        file=fat:rw:bin,format=raw,media=disk
        -serial
        ${QEMU_NORMAL_WORLD_DEV_PATH}
        -serial
        ${QEMU_SECURE_WORLD_DEV_PATH}
        -bios
        ${arm-trusted-firmware_BINARY_DIR}/flash.bin
        -kernel
        $<TARGET_FILE:kernel>)
ENDIF()

GEN_DTB_DTS (
    WORKING_DIRECTORY
    ${PROJECT_BINARY_DIR}
    TARGET
    ${CMAKE_SYSTEM_PROCESSOR}
    QEMU_FLAGS
    ${QEMU_COMMON_FLAG}
    ${QEMU_MACHINE_FLAGS})

GEN_FIT (
    DEPENDS
    kernel
    gen_dtb_dts
    WORKING_DIRECTORY
    ${PROJECT_BINARY_DIR}
    TARGET
    ${CMAKE_SYSTEM_PROCESSOR})

# 添加 run 和 debug target
ADD_RUN_TARGET (
    DEPENDS
    ovmf
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:gen_fit>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:gen_fit>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:opensbi>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:u-boot>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},x86_64>:boot>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:optee_os>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:arm-trusted-firmware>
    kernel
    WORKING_DIRECTORY
    ${PROJECT_BINARY_DIR}
    TARGET
    ${CMAKE_SYSTEM_PROCESSOR}
    BOOT
    ${PROJECT_BINARY_DIR}/src/boot/boot.efi
    KERNEL
    $<TARGET_FILE:kernel>
    QEMU_FLAGS
    ${QEMU_COMMON_FLAG}
    ${QEMU_MACHINE_FLAGS}
    ${QEMU_BOOT_FLAGS})
