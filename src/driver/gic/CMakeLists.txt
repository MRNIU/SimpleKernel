# Copyright The SimpleKernel Contributors
# CMake build configuration

ADD_LIBRARY (gic INTERFACE)

TARGET_INCLUDE_DIRECTORIES (gic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (gic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/gic.cpp)
