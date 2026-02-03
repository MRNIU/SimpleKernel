# Copyright The SimpleKernel Contributors

ADD_LIBRARY (driver INTERFACE)

TARGET_INCLUDE_DIRECTORIES (driver INTERFACE include)

TARGET_SOURCES (driver INTERFACE driver.cpp)

TARGET_LINK_LIBRARIES (
    driver
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
    ADD_SUBDIRECTORY (ns16550a)
    ADD_SUBDIRECTORY (plic)
ELSEIF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "aarch64")
    ADD_SUBDIRECTORY (pl011)
    ADD_SUBDIRECTORY (gic)
ELSEIF(${CMAKE_SYSTEM_PROCESSOR} STREQUAL "x86_64")
    ADD_SUBDIRECTORY (apic)
    ADD_SUBDIRECTORY (acpi)
ENDIF()
