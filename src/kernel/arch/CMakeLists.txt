# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

ADD_LIBRARY (arch INTERFACE)

TARGET_INCLUDE_DIRECTORIES (
    arch
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}
              ${CMAKE_CURRENT_SOURCE_DIR}/${CMAKE_SYSTEM_PROCESSOR}/include)

TARGET_SOURCES (
    arch
    INTERFACE
        $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:
        ${CMAKE_CURRENT_SOURCE_DIR}/${CMAKE_SYSTEM_PROCESSOR}/macro.S
        ${CMAKE_CURRENT_SOURCE_DIR}/${CMAKE_SYSTEM_PROCESSOR}/boot.S
        >
        $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:
        ${CMAKE_CURRENT_SOURCE_DIR}/${CMAKE_SYSTEM_PROCESSOR}/boot.S
        >
        ${CMAKE_CURRENT_SOURCE_DIR}/${CMAKE_SYSTEM_PROCESSOR}/arch_main.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/${CMAKE_SYSTEM_PROCESSOR}/backtrace.cpp)
