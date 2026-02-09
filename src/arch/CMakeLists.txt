# Copyright The SimpleKernel Contributors

ADD_LIBRARY (arch INTERFACE)

TARGET_INCLUDE_DIRECTORIES (arch INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}
                                           ${CMAKE_SYSTEM_PROCESSOR}/include)

TARGET_SOURCES (
    arch
    INTERFACE ${CMAKE_SYSTEM_PROCESSOR}/boot.S
              ${CMAKE_SYSTEM_PROCESSOR}/arch_main.cpp
              ${CMAKE_SYSTEM_PROCESSOR}/early_console.cpp
              ${CMAKE_SYSTEM_PROCESSOR}/backtrace.cpp
              ${CMAKE_SYSTEM_PROCESSOR}/interrupt.S
              ${CMAKE_SYSTEM_PROCESSOR}/switch.S
              ${CMAKE_SYSTEM_PROCESSOR}/interrupt.cpp
              ${CMAKE_SYSTEM_PROCESSOR}/timer.cpp
              ${CMAKE_SYSTEM_PROCESSOR}/interrupt_main.cpp
              ${CMAKE_SYSTEM_PROCESSOR}/syscall.cpp)
