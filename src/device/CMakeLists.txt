# Copyright The SimpleKernel Contributors

ADD_LIBRARY (device INTERFACE)

TARGET_INCLUDE_DIRECTORIES (device
                            INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (device INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/device.cpp)

TARGET_LINK_LIBRARIES (
    device
    INTERFACE $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:
              ns16550a
              plic>
              $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:
              pl011
              gic>
              $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},x86_64>:
              apic
              acpi>)

IF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "riscv64")
    ADD_SUBDIRECTORY (driver/ns16550a)
    ADD_SUBDIRECTORY (driver/plic)
ELSEIF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
    ADD_SUBDIRECTORY (driver/pl011)
    ADD_SUBDIRECTORY (driver/gic)
ELSEIF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "x86_64")
    ADD_SUBDIRECTORY (driver/apic)
    ADD_SUBDIRECTORY (driver/acpi)
ENDIF()
