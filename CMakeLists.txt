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

# qemu 参数设置
LIST (
    APPEND
    QEMU_FLAGS
    # 不启用图形界面
    -nographic
    # 使用标准输出显示
    -serial
    stdio
    # 启动 telnet 服务，使用 2333 端口，不等待连接
    -monitor
    ${QEMU_MONITOR_ARG})
# 目标平台参数
IF(CMAKE_SYSTEM_PROCESSOR STREQUAL "x86_64")
    LIST (
        APPEND
        QEMU_FLAGS
        -m
        128M
        -net
        none
        -bios
        ${ovmf_BINARY_DIR}/OVMF_${CMAKE_SYSTEM_PROCESSOR}.fd
        -hda
        fat:rw:./image/)
ELSEIF(CMAKE_SYSTEM_PROCESSOR STREQUAL "riscv64")
    LIST (
        APPEND
        QEMU_FLAGS
        -machine
        virt
        # 可选项，qemu7.0 自带了 opensbi1.0
        -bios
        ${opensbi_BINARY_DIR}/platform/generic/firmware/fw_jump.elf
        -kernel
        $<TARGET_FILE:kernel>
        -smp
        4
        # @todo 暂时还不支持 riscv64 的 uefi 启动 预期路线: qemu->uefi->opensbi +
        # kernel(payload) -bios
        # ${ovmf_BINARY_DIR}/OVMF_${CMAKE_SYSTEM_PROCESSOR}.fd -hda
        # fat:rw:./image/
    )
ELSEIF(CMAKE_SYSTEM_PROCESSOR STREQUAL "aarch64")
    LIST (
        APPEND
        QEMU_FLAGS
        -machine
        virt
        -cpu
        cortex-a72
        -m
        128M
        -net
        none
        -kernel
        $<TARGET_FILE:kernel>
        # -bios
        # ${ovmf_BINARY_DIR}/OVMF_${CMAKE_SYSTEM_PROCESSOR}.fd
        # -hda
        # fat:rw:./image/
    )
ENDIF()

# 添加要编译的目录
ADD_SUBDIRECTORY (${PROJECT_SOURCE_DIR}/src)
ADD_SUBDIRECTORY (${PROJECT_SOURCE_DIR}/test)
ADD_SUBDIRECTORY (${PROJECT_SOURCE_DIR}/doc)

# 添加 run 和 debug target
ADD_RUN_TARGET (
    DEPENDS
    ovmf
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:opensbi-fw_jump>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},x86_64>:boot>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:boot>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:boot>
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
    ${QEMU_FLAGS})
