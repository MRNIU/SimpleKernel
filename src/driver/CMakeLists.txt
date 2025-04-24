# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

ADD_LIBRARY (driver INTERFACE)

TARGET_INCLUDE_DIRECTORIES (driver
                            INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (driver INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/driver.cpp)

TARGET_LINK_LIBRARIES (
    driver INTERFACE $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>: ns16550a
                     > $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>: pl011 >)

IF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "riscv64")
    ADD_SUBDIRECTORY (ns16550a)
ELSEIF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
    ADD_SUBDIRECTORY (pl011)
ENDIF()
