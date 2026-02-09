# Copyright The SimpleKernel Contributors

IF(NOT UNIX)
    MESSAGE (FATAL_ERROR "Only support Linux.")
ENDIF()

IF(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "aarch64")
    FIND_PROGRAM (Compiler_gcc g++)
    IF(NOT Compiler_gcc)
        MESSAGE (
            FATAL_ERROR "g++ not found.\n"
                        "Run `sudo apt-get install -y gcc g++` to install.")
    ELSE()
        MESSAGE (STATUS "Found g++ ${Compiler_gcc}")
    ENDIF()

    SET (TOOLCHAIN_PREFIX32 arm-linux-gnueabihf-)
ELSEIF(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "x86_64")
    FIND_PROGRAM (Compiler_gcc aarch64-linux-gnu-g++)
    IF(NOT Compiler_gcc)
        MESSAGE (
            FATAL_ERROR
                "aarch64-linux-gnu-g++ not found.\n"
                "Run `sudo apt install -y gcc-aarch64-linux-gnu g++-aarch64-linux-gnu` to install."
        )
    ELSE()
        MESSAGE (STATUS "Found aarch64-linux-gnu-g++ ${Compiler_gcc}")
    ENDIF()

    SET (TOOLCHAIN_PREFIX aarch64-linux-gnu-)
    SET (CMAKE_C_COMPILER ${TOOLCHAIN_PREFIX}gcc)
    SET (CMAKE_CXX_COMPILER ${TOOLCHAIN_PREFIX}g++)
    SET (CMAKE_READELF ${TOOLCHAIN_PREFIX}readelf)
    SET (CMAKE_AR ${TOOLCHAIN_PREFIX}ar)
    SET (CMAKE_LINKER ${TOOLCHAIN_PREFIX}ld)
    SET (CMAKE_NM ${TOOLCHAIN_PREFIX}nm)
    SET (CMAKE_OBJDUMP ${TOOLCHAIN_PREFIX}objdump)
    SET (CMAKE_RANLIB ${TOOLCHAIN_PREFIX}ranlib)
    SET (TOOLCHAIN_PREFIX32 arm-linux-gnueabihf-)
ELSE()
    MESSAGE (FATAL_ERROR "NOT support ${CMAKE_HOST_SYSTEM_PROCESSOR}")
ENDIF()
