# Copyright The SimpleKernel Contributors
# CMake build configuration

ADD_LIBRARY (ns16550a INTERFACE)

TARGET_INCLUDE_DIRECTORIES (ns16550a INTERFACE include)

TARGET_SOURCES (ns16550a INTERFACE ns16550a.cpp)
