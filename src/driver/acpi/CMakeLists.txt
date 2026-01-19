# Copyright The SimpleKernel Contributors

ADD_LIBRARY (acpi INTERFACE)

TARGET_INCLUDE_DIRECTORIES (acpi INTERFACE include)

TARGET_SOURCES (acpi INTERFACE acpi.cpp)
