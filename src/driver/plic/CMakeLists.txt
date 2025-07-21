# Copyright The SimpleKernel Contributors
# CMake build configuration

ADD_LIBRARY (plic INTERFACE)

TARGET_INCLUDE_DIRECTORIES (plic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (plic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/plic.cpp)
