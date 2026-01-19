# Copyright The SimpleKernel Contributors
# CMake build configuration

ADD_LIBRARY (acpi INTERFACE)

TARGET_INCLUDE_DIRECTORIES (acpi INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (acpi INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/acpi.cpp)
