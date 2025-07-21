# Copyright The SimpleKernel Contributors
# CMake build configuration

ADD_LIBRARY (libcxx INTERFACE)

TARGET_INCLUDE_DIRECTORIES (libcxx
                            INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (
    libcxx
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/sk_libcxx.cpp
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_new.cpp
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_iostream.cpp)
