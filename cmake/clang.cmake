# Copyright The SimpleKernel Contributors

# 目标为无操作系统的环境
SET (CMAKE_SYSTEM_NAME Generic)
# 目标架构
SET (CMAKE_SYSTEM_PROCESSOR x86_64)

# @todo mac 测试
IF(APPLE)
    MESSAGE (STATUS "Now is Apple systems.")
    # @todo
ELSEIF(UNIX)
    MESSAGE (STATUS "Now is UNIX-like OS's.")
    # clang
    FIND_PROGRAM (Compiler_clang++ clang++)
    IF(NOT Compiler_clang++)
        MESSAGE (
            FATAL_ERROR
                "clang++ not found.\n"
                "Run `sudo apt-get install -y clang clang++` to install.")
    ELSE()
        MESSAGE (STATUS "Found clang++ ${Compiler_clang++}")
    ENDIF()

    SET (CMAKE_C_COMPILER clang)
    SET (CMAKE_CXX_COMPILER clang++)
    SET (CMAKE_READELF readelf)
    SET (CMAKE_AR ar)
    SET (CMAKE_LINKER ld)
    SET (CMAKE_NM nm)
    SET (CMAKE_OBJDUMP objdump)
    SET (CMAKE_RANLIB ranlib)

    # qemu
    FIND_PROGRAM (QEMU qemu-system-x86_64)
    IF(NOT QEMU)
        MESSAGE (
            FATAL_ERROR "qemu not found.\n"
                        "Run `sudo apt-get install -y qemu-system` to install.")
    ELSE()
        MESSAGE (STATUS "Found qemu ${QEMU}")
    ENDIF()
ENDIF()
