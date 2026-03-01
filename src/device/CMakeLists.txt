# Copyright The SimpleKernel Contributors

ADD_LIBRARY (device INTERFACE)

TARGET_INCLUDE_DIRECTORIES (device BEFORE
                            INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (device INTERFACE device.cpp)
