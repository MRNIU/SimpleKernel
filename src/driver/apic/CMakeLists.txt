# Copyright The SimpleKernel Contributors
# CMake build configuration

ADD_LIBRARY (apic INTERFACE)

TARGET_INCLUDE_DIRECTORIES (apic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (apic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/apic.cpp)
