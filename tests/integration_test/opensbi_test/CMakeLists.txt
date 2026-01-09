# Copyright The SimpleKernel Contributors

PROJECT (opensbi-test)

ENABLE_LANGUAGE (ASM)
ENABLE_LANGUAGE (CXX)

ADD_EXECUTABLE (
    ${PROJECT_NAME}
    main.cpp ${CMAKE_SOURCE_DIR}/src/arch/${CMAKE_SYSTEM_PROCESSOR}/boot.S)

# 添加要链接的库
TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE kernel_link_libraries libc
                                               libcxx)

# qemu 参数设置
LIST (
    APPEND
    ${PROJECT_NAME}_QEMU_BOOT_FLAGS
    "-D;${CMAKE_BINARY_DIR}/bin/${PROJECT_NAME}_qemu.log -bios
        ${u-boot_BINARY_DIR}/spl/u-boot-spl.bin -device
        loader,file=${u-boot_BINARY_DIR}/u-boot.itb,addr=0x80200000")

# 添加 run 和 debug target
ADD_RUN_TARGET (
    NAME
    ${PROJECT_NAME}_
    TARGET
    ${PROJECT_NAME}
    DEPENDS
    u-boot
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:opensbi>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:optee_os>
    $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:arm-trusted-firmware>
    QEMU_BOOT_FLAGS
    ${${PROJECT_NAME}_QEMU_BOOT_FLAGS})
