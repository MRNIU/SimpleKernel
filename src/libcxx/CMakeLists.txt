# Copyright The SimpleKernel Contributors

ADD_LIBRARY (libcxx INTERFACE)

TARGET_INCLUDE_DIRECTORIES (libcxx INTERFACE include)

TARGET_SOURCES (libcxx INTERFACE kstd_libcxx.cpp kstd_new.cpp)

# klog_format.cpp must be compiled WITHOUT -mgeneral-regs-only because
# etl::vformat_to instantiates floating-point formatting templates.
# We use a separate OBJECT library so it does not inherit that flag
# from the INTERFACE compile_options target.
ADD_LIBRARY (klog_format_obj OBJECT klog_format.cpp)
TARGET_INCLUDE_DIRECTORIES (
    klog_format_obj PRIVATE include ${CMAKE_CURRENT_SOURCE_DIR}/../include
                            ${CMAKE_CURRENT_SOURCE_DIR}/../libc/include)
TARGET_LINK_LIBRARIES (
    klog_format_obj PRIVATE compile_definitions 3rd_compile_definitions
                            etl::etl cpu_io MPMCQueue)
TARGET_COMPILE_OPTIONS (
    klog_format_obj
    PRIVATE # Mirror compile_options but WITHOUT -mgeneral-regs-only
            $<$<CONFIG:Release>:-O2;-Werror>
            $<$<CONFIG:Debug>:-O0;-ggdb>
            -Wall
            -Wextra
            -pedantic
            -fPIC
            -fPIE
            $<$<COMPILE_LANGUAGE:CXX>:-fno-rtti>
            -fno-exceptions
            -ffreestanding
            -fno-omit-frame-pointer
            -fno-common
            -fcheck-new
            # x86_64: no -mgeneral-regs-only, keep -mno-red-zone
            $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},x86_64>:
            -mno-red-zone
            >
            # riscv64: same as compile_options (no -mgeneral-regs-only there)
            $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:
            -mstrict-align
            -march=rv64gc_zihintpause
            >
            # aarch64: no -mgeneral-regs-only, keep other flags
            $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:
            -mstrict-align
            -march=armv8-a
            -mtune=cortex-a72
            -mno-outline-atomics
            >
            ${CROSS_INCLUDE_PATHS})
