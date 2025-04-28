# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# 3rd.cmake for Simple-XX/SimpleKernel.
# 依赖管理

# # https://github.com/abumq/easyloggingpp
# CPMAddPackage(
#   NAME easylogingpp
#   VERSION 9.97.0
#   GITHUB_REPOSITORY amrayn/easyloggingpp
#   OPTIONS
#   "build_static_lib ON"
#   "lib_utc_datetime ON"
# )

# https://github.com/rttrorg/rttr
# @bug 打开这个会导致编译参数中多出来几个
# CPMAddPackage(
#   NAME rttr # link against RTTR::Core_Lib
#   VERSION 0.9.6
#   GITHUB_REPOSITORY rttrorg/rttr
#   OPTIONS
#     "BUILD_RTTR_DYNAMIC Off"
#     "BUILD_UNIT_TESTS Off"
#     "BUILD_STATIC On"
#     "BUILD_PACKAGE Off"
#     "BUILD_WITH_RTTI On"
#     "BUILD_EXAMPLES Off"
#     "BUILD_DOCUMENTATION Off"
#     "BUILD_INSTALLER Off"
#     "USE_PCH Off"
#     "CUSTOM_DOXYGEN_STYLE Off"
# )

# https://github.com/TheLartians/Format.cmake
# CPMAddPackage(
#   NAME Format.cmake
#   GITHUB_REPOSITORY TheLartians/Format.cmake
#   VERSION 1.7.3
# )

# # https://github.com/freetype/freetype
# CPMAddPackage(
#   NAME freetype
#   GIT_REPOSITORY https://github.com/freetype/freetype.git
#   GIT_TAG VER-2-13-0
#   VERSION 2.13.0
# )
# if (freetype_ADDED)
#   add_library(Freetype::Freetype ALIAS freetype)
# endif()

# Pre-commit hooks
IF(NOT EXISTS ${CMAKE_SOURCE_DIR}/.git/hooks/pre-commit)
    EXECUTE_PROCESS (COMMAND pre-commit install)
ENDIF()

# https://github.com/google/googletest.git
IF(NOT TARGET gtest)
    ADD_SUBDIRECTORY (3rd/googletest)
    INCLUDE (GoogleTest)
ENDIF()

# https://git.savannah.gnu.org/git/grub.git
SET (grub2_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/grub2)
ADD_LIBRARY (grub2 INTERFACE)
TARGET_INCLUDE_DIRECTORIES (grub2 INTERFACE ${grub2_SOURCE_DIR}/include)

# https://github.com/gdbinit/Gdbinit.git
SET (gdbinit_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/gdbinit)
SET (gdbinit_BINARY_DIR ${CMAKE_BINARY_DIR}/3rd/gdbinit)
ADD_CUSTOM_TARGET (
    gdbinit
    COMMENT "Generate gdbinit ..."
    WORKING_DIRECTORY ${gdbinit_SOURCE_DIR}
    # 复制到根目录下并重命名
    COMMAND ln -s -f ${gdbinit_SOURCE_DIR}/gdbinit ${CMAKE_SOURCE_DIR}/.gdbinit
    COMMAND echo "target remote ${QEMU_GDB_PORT}" >>
            ${CMAKE_SOURCE_DIR}/.gdbinit
    COMMAND
        echo "add-symbol-file ${kernel_BINARY_DIR}/${KERNEL_ELF_OUTPUT_NAME}" >>
        ${CMAKE_SOURCE_DIR}/.gdbinit
    COMMAND echo "add-symbol-file ${boot_BINARY_DIR}/${BOOT_ELF_OUTPUT_NAME}"
            >> ${CMAKE_SOURCE_DIR}/.gdbinit)
# 在 make clean 时删除 .gdbinit
SET_DIRECTORY_PROPERTIES (PROPERTIES ADDITIONAL_MAKE_CLEAN_FILES
                                     ${CMAKE_SOURCE_DIR}/.gdbinit)

# https://github.com/eyalroz/printf.git
ADD_SUBDIRECTORY (3rd/printf)

# https://github.com/MRNIU/cpu_io.git
ADD_SUBDIRECTORY (3rd/cpu_io)

# https://github.com/buildroot/buildroot.git
SET (buildroot_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/buildroot)
SET (buildroot_BINARY_DIR ${CMAKE_BINARY_DIR}/3rd/buildroot)
ADD_CUSTOM_TARGET (
    buildroot
    COMMENT "build buildroot..."
    # make 时编译
    ALL
    WORKING_DIRECTORY ${buildroot_SOURCE_DIR}
    COMMAND ${CMAKE_COMMAND} -E make_directory ${buildroot_BINARY_DIR}
    COMMAND make O=${buildroot_BINARY_DIR} qemu_aarch64_virt_defconfig
            -j${CMAKE_BUILD_PARALLEL_LEVEL}
    COMMAND cd ${buildroot_BINARY_DIR} && ${buildroot_SOURCE_DIR}/utils/config
            -e BR2_TARGET_ROOTFS_CPIO
    COMMAND cd ${buildroot_BINARY_DIR} && ${buildroot_SOURCE_DIR}/utils/config
            -e BR2_TARGET_ROOTFS_CPIO_GZIP
    COMMAND cd ${buildroot_BINARY_DIR} && make olddefconfig
            -j${CMAKE_BUILD_PARALLEL_LEVEL}
    COMMAND cd ${buildroot_BINARY_DIR} && make -j${CMAKE_BUILD_PARALLEL_LEVEL})
SET_DIRECTORY_PROPERTIES (PROPERTIES ADDITIONAL_MAKE_CLEAN_FILES
                                     ${buildroot_BINARY_DIR})

IF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "riscv64")
    # https://github.com/riscv-software-src/opensbi.git
    # 编译 opensbi
    SET (opensbi_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/opensbi)
    SET (opensbi_BINARY_DIR ${CMAKE_BINARY_DIR}/3rd/opensbi)
    ADD_CUSTOM_TARGET (
        opensbi
        COMMENT "build opensbi..."
        # make 时编译
        ALL
        WORKING_DIRECTORY ${opensbi_SOURCE_DIR}
        COMMAND ${CMAKE_COMMAND} -E make_directory ${opensbi_BINARY_DIR}
        COMMAND
            make PLATFORM_RISCV_XLEN=64 PLATFORM=generic FW_JUMP_ADDR=0x80210000
            CROSS_COMPILE=${TOOLCHAIN_PREFIX} O=${opensbi_BINARY_DIR}
            -j${CMAKE_BUILD_PARALLEL_LEVEL}
        COMMAND ln -s -f ${opensbi_SOURCE_DIR}/include ${opensbi_BINARY_DIR})

    # https://github.com/MRNIU/opensbi_interface.git
    ADD_SUBDIRECTORY (3rd/opensbi_interface)
ENDIF()

# https://github.com/u-boot/u-boot.git
SET (u-boot_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/u-boot)
SET (u-boot_BINARY_DIR ${CMAKE_BINARY_DIR}/3rd/u-boot)
ADD_CUSTOM_TARGET (
    u-boot
    COMMENT "build u-boot..."
    # make 时编译
    ALL
    DEPENDS $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:opensbi>
    WORKING_DIRECTORY ${u-boot_SOURCE_DIR}
    COMMAND ${CMAKE_COMMAND} -E make_directory ${u-boot_BINARY_DIR}
    COMMAND
        make O=${u-boot_BINARY_DIR}
        $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:qemu_arm64_defconfig>
        $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:qemu-riscv64_spl_defconfig>
        $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},x86_64>:qemu-x86_64_defconfig>
        -j${CMAKE_BUILD_PARALLEL_LEVEL}
    COMMAND
        make CROSS_COMPILE=${TOOLCHAIN_PREFIX} O=${u-boot_BINARY_DIR}
        $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:OPENSBI=${opensbi_BINARY_DIR}/platform/generic/firmware/fw_dynamic.bin>
        -j${CMAKE_BUILD_PARALLEL_LEVEL})
SET_DIRECTORY_PROPERTIES (PROPERTIES ADDITIONAL_MAKE_CLEAN_FILES
                                     ${u-boot_BINARY_DIR})

IF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
    # https://github.com/OP-TEE/optee_os.git
    SET (optee_os_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/optee/optee_os)
    SET (optee_os_BINARY_DIR ${CMAKE_BINARY_DIR}/3rd/optee/optee_os)
    ADD_CUSTOM_TARGET (
        optee_os
        COMMENT "build optee_os..."
        # make 时编译
        ALL
        WORKING_DIRECTORY ${optee_os_SOURCE_DIR}
        COMMAND ${CMAKE_COMMAND} -E make_directory ${optee_os_BINARY_DIR}
        COMMAND
            make CFG_ARM64_core=y CFG_TEE_BENCHMARK=n CFG_TEE_CORE_LOG_LEVEL=3
            CROSS_COMPILE=${TOOLCHAIN_PREFIX}
            CROSS_COMPILE_core=${TOOLCHAIN_PREFIX}
            CROSS_COMPILE_ta_arm32=${TOOLCHAIN_PREFIX32}
            CROSS_COMPILE_ta_arm64=${TOOLCHAIN_PREFIX} DEBUG=$<CONFIG:Debug>·
            O=${optee_os_BINARY_DIR} PLATFORM=vexpress-qemu_armv8a
            CFG_ARM_GICV3=y -j${CMAKE_BUILD_PARALLEL_LEVEL})
    SET_DIRECTORY_PROPERTIES (PROPERTIES ADDITIONAL_MAKE_CLEAN_FILES
                                         ${optee_os_BINARY_DIR})

    # https://github.com/OP-TEE/optee_client.git
    ADD_SUBDIRECTORY (${CMAKE_SOURCE_DIR}/3rd/optee/optee_client)

    # https://github.com/ARM-software/arm-trusted-firmware
    # 编译 atf
    SET (arm-trusted-firmware_SOURCE_DIR
         ${CMAKE_SOURCE_DIR}/3rd/arm-trusted-firmware)
    SET (arm-trusted-firmware_BINARY_DIR
         ${CMAKE_BINARY_DIR}/3rd/arm-trusted-firmware)
    ADD_CUSTOM_TARGET (
        arm-trusted-firmware
        COMMENT "build arm-trusted-firmware..."
        # make 时编译
        ALL
        DEPENDS optee_os u-boot
        WORKING_DIRECTORY ${arm-trusted-firmware_SOURCE_DIR}
        COMMAND ${CMAKE_COMMAND} -E make_directory
                ${arm-trusted-firmware_BINARY_DIR}
        COMMAND
            make DEBUG=$<CONFIG:Debug> CROSS_COMPILE=${TOOLCHAIN_PREFIX}
            PLAT=qemu BUILD_BASE=${arm-trusted-firmware_BINARY_DIR}
            BL32=${optee_os_BINARY_DIR}/core/tee-header_v2.bin
            BL32_EXTRA1=${optee_os_BINARY_DIR}/core/tee-pager_v2.bin
            BL32_EXTRA2=${optee_os_BINARY_DIR}/core/tee-pageable_v2.bin
            BL33=${u-boot_BINARY_DIR}/u-boot.bin BL32_RAM_LOCATION=tdram
            QEMU_USE_GIC_DRIVER=QEMU_GICV3 SPD=opteed all fip
            -j${CMAKE_BUILD_PARALLEL_LEVEL}
        COMMAND
            dd
            if=${arm-trusted-firmware_BINARY_DIR}/qemu/$<IF:$<CONFIG:Debug>,debug,release>/bl1.bin
            of=${arm-trusted-firmware_BINARY_DIR}/flash.bin bs=4096 conv=notrunc
        COMMAND
            dd
            if=${arm-trusted-firmware_BINARY_DIR}/qemu/$<IF:$<CONFIG:Debug>,debug,release>/fip.bin
            of=${arm-trusted-firmware_BINARY_DIR}/flash.bin seek=64 bs=4096
            conv=notrunc)
    SET_DIRECTORY_PROPERTIES (PROPERTIES ADDITIONAL_MAKE_CLEAN_FILES
                                         ${arm-trusted-firmware_BINARY_DIR})
ENDIF()

# https://git.kernel.org/pub/scm/utils/dtc/dtc.git
SET (dtc_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/dtc)
SET (dtc_BINARY_DIR ${CMAKE_BINARY_DIR}/3rd/dtc)
SET (dtc_CC ${CMAKE_C_COMPILER})
SET (dtc_AR ${CMAKE_AR})
# 编译 libfdt
IF(NOT EXISTS ${dtc_BINARY_DIR}/libfdt/libfdt.a)
    ADD_CUSTOM_TARGET (
        dtc
        COMMENT "build libdtc..."
        # make 时编译
        ALL
        WORKING_DIRECTORY ${dtc_SOURCE_DIR}
        COMMAND ${CMAKE_COMMAND} -E make_directory ${dtc_BINARY_DIR}/libfdt
        COMMAND CC=${dtc_CC} AR=${dtc_AR} HOME=${dtc_BINARY_DIR} make
                libfdt/libfdt.a -j${CMAKE_BUILD_PARALLEL_LEVEL}
        COMMAND ${CMAKE_COMMAND} -E copy ${dtc_SOURCE_DIR}/libfdt/*.a
                ${dtc_SOURCE_DIR}/libfdt/*.h ${dtc_BINARY_DIR}/libfdt
        COMMAND make clean)
ELSE()
    ADD_CUSTOM_TARGET (
        dtc
        COMMENT "libdtc already exists, skipping..."
        # make 时编译
        ALL
        WORKING_DIRECTORY ${dtc_SOURCE_DIR})
ENDIF()
ADD_LIBRARY (dtc-lib INTERFACE)
ADD_DEPENDENCIES (dtc-lib dtc)
TARGET_INCLUDE_DIRECTORIES (dtc-lib INTERFACE ${dtc_BINARY_DIR}/libfdt)
TARGET_LINK_LIBRARIES (dtc-lib INTERFACE ${dtc_BINARY_DIR}/libfdt/libfdt.a)

# https://github.com/ncroxon/gnu-efi.git
SET (gnu-efi_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/gnu-efi)
SET (gnu-efi_BINARY_DIR ${CMAKE_BINARY_DIR}/3rd/gnu-efi)
IF(CMAKE_SYSTEM_PROCESSOR STREQUAL CMAKE_HOST_SYSTEM_PROCESSOR)
    SET (CC_ ${CMAKE_C_COMPILER})
    SET (AR_ ${CMAKE_AR})
ELSEIF(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "aarch64" AND CMAKE_SYSTEM_PROCESSOR
                                                         MATCHES "x86_64")
    SET (CROSS_COMPILE_ x86_64-linux-gnu-)
ELSEIF(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "aarch64" AND CMAKE_SYSTEM_PROCESSOR
                                                         MATCHES "riscv64")
    SET (CROSS_COMPILE_ riscv64-linux-gnu-)
ELSEIF(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "x86_64" AND CMAKE_SYSTEM_PROCESSOR
                                                        MATCHES "riscv64")
    SET (CROSS_COMPILE_ riscv64-linux-gnu-)
ELSEIF(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "x86_64" AND CMAKE_SYSTEM_PROCESSOR
                                                        MATCHES "aarch64")
    SET (CROSS_COMPILE_ aarch64-linux-gnu-)
ENDIF()
IF(NOT EXISTS ${gnu-efi_BINARY_DIR}/lib/libefi.a)
    # 编译 gnu-efi
    ADD_CUSTOM_TARGET (
        gnu-efi
        COMMENT "build gnu-efi..."
        # make 时编译
        ALL
        WORKING_DIRECTORY ${gnu-efi_SOURCE_DIR}
        COMMAND ${CMAKE_COMMAND} -E make_directory ${gnu-efi_BINARY_DIR}
        COMMAND
            # @note 仅支持 gcc
            make lib gnuefi inc CROSS_COMPILE=${CROSS_COMPILE_}
            ARCH=${CMAKE_SYSTEM_PROCESSOR} OBJDIR=${gnu-efi_BINARY_DIR} V=1
            -j${CMAKE_BUILD_PARALLEL_LEVEL}
        COMMAND ${CMAKE_COMMAND} -E copy_directory ${gnu-efi_SOURCE_DIR}/inc
                ${gnu-efi_BINARY_DIR}/inc)
ELSE()
    ADD_CUSTOM_TARGET (
        gnu-efi
        COMMENT "gnu-efi already exists, skipping..."
        ALL
        WORKING_DIRECTORY ${gnu-efi_SOURCE_DIR})
ENDIF()
ADD_LIBRARY (gnu-efi-lib INTERFACE)
ADD_DEPENDENCIES (gnu-efi-lib gnu-efi)
TARGET_INCLUDE_DIRECTORIES (
    gnu-efi-lib
    INTERFACE ${gnu-efi_BINARY_DIR}/inc
              ${gnu-efi_BINARY_DIR}/inc/${CMAKE_SYSTEM_PROCESSOR}
              ${gnu-efi_BINARY_DIR}/inc/protocol)
TARGET_LINK_LIBRARIES (
    gnu-efi-lib
    INTERFACE ${gnu-efi_BINARY_DIR}/gnuefi/reloc_${CMAKE_SYSTEM_PROCESSOR}.o
              ${gnu-efi_BINARY_DIR}/gnuefi/crt0-efi-${CMAKE_SYSTEM_PROCESSOR}.o
              ${gnu-efi_BINARY_DIR}/gnuefi/libgnuefi.a
              ${gnu-efi_BINARY_DIR}/lib/libefi.a)

# gdb
FIND_PROGRAM (GDB_EXE gdb)
IF(NOT GDB_EXE)
    MESSAGE (
        FATAL_ERROR "gdb not found.\n"
                    "Following https://www.sourceware.org/gdb/ to install.")
ENDIF()

# qemu
FIND_PROGRAM (QEMU_EXE qemu-system-${CMAKE_SYSTEM_PROCESSOR})
IF(NOT QEMU_EXE)
    MESSAGE (FATAL_ERROR "qemu-system-${CMAKE_SYSTEM_PROCESSOR} not found.\n"
                         "Following https://www.qemu.org/ to install.")
ENDIF()

# doxygen
FIND_PACKAGE (Doxygen REQUIRED dot)
IF(NOT DOXYGEN_FOUND)
    MESSAGE (
        FATAL_ERROR "Doxygen not found.\n"
                    "Following https://www.doxygen.nl/index.html to install.")
ENDIF()

# cppcheck
FIND_PROGRAM (CPPCHECK_EXE NAMES cppcheck)
IF(NOT CPPCHECK_EXE)
    MESSAGE (
        FATAL_ERROR "cppcheck not found.\n"
                    "Following https://cppcheck.sourceforge.io to install.")
ENDIF()
ADD_CUSTOM_TARGET (
    cppcheck
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    COMMENT "Run cppcheck on ${CMAKE_BINARY_DIR}/compile_commands.json ..."
    COMMAND
        ${CPPCHECK_EXE} --enable=all
        --project=${CMAKE_BINARY_DIR}/compile_commands.json
        --suppress-xml=${CMAKE_SOURCE_DIR}/tools/cppcheck-suppressions.xml
        --output-file=${CMAKE_BINARY_DIR}/cppcheck_report.log)

IF(CMAKE_SYSTEM_PROCESSOR STREQUAL CMAKE_HOST_SYSTEM_PROCESSOR)
    # genhtml 生成测试覆盖率报告网页
    FIND_PROGRAM (GENHTML_EXE genhtml)
    IF(NOT GENHTML_EXE)
        MESSAGE (
            FATAL_ERROR
                "genhtml not found.\n"
                "Following https://github.com/linux-test-project/lcov to install."
        )
    ENDIF()

    # lcov 生成测试覆盖率报告
    FIND_PROGRAM (LCOV_EXE lcov)
    IF(NOT LCOV_EXE)
        MESSAGE (
            FATAL_ERROR
                "lcov not found.\n"
                "Following https://github.com/linux-test-project/lcov to install."
        )
    ENDIF()
ENDIF()
