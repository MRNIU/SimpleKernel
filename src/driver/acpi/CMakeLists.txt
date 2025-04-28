# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

ADD_LIBRARY (acpi INTERFACE)

TARGET_INCLUDE_DIRECTORIES (acpi INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (acpi INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/acpi.cpp)
