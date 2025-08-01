# Copyright The SimpleKernel Contributors

ADD_LIBRARY (apic INTERFACE)

TARGET_INCLUDE_DIRECTORIES (apic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (
    apic
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/apic.cpp
              ${CMAKE_CURRENT_SOURCE_DIR}/local_apic.cpp
              ${CMAKE_CURRENT_SOURCE_DIR}/io_apic.cpp)
