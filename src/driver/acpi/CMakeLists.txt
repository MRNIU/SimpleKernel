# Copyright The SimpleKernel Contributors
# CMake build configuration

ADD_LIBRARY (acpi INTERFACE)

TARGET_INCLUDE_DIRECTORIES (acpi INTERFACE include)

TARGET_SOURCES (acpi INTERFACE acpi.cpp)
