# Copyright The SimpleKernel Contributors

ADD_LIBRARY (libc INTERFACE)

TARGET_INCLUDE_DIRECTORIES (libc INTERFACE include)

TARGET_SOURCES (libc INTERFACE sk_stdlib.c sk_string.c sk_ctype.c)

# sk_math.c must be compiled WITHOUT -mgeneral-regs-only because it
# provides FP math stubs (pow, modf, etc.) needed by ETL format templates.
ADD_LIBRARY (sk_math_obj OBJECT sk_math.c)
TARGET_INCLUDE_DIRECTORIES (sk_math_obj PRIVATE include)
TARGET_LINK_LIBRARIES (sk_math_obj PRIVATE compile_definitions)
TARGET_COMPILE_OPTIONS (
    sk_math_obj
    PRIVATE $<$<CONFIG:Release>:-O2;-Werror>
            $<$<CONFIG:Debug>:-O0;-ggdb>
            -Wall
            -Wextra
            -pedantic
            -fPIC
            -fPIE
            -fno-exceptions
            -ffreestanding
            -fno-omit-frame-pointer
            -fno-common
            # x86_64: no -mgeneral-regs-only, keep -mno-red-zone
            $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},x86_64>:
            -mno-red-zone
            >
            # riscv64: same as compile_options
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
