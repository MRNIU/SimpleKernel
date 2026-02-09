<<<<<<< HEAD
# Copyright The SimpleKernel Contributors

ADD_LIBRARY (libcxx INTERFACE)

TARGET_INCLUDE_DIRECTORIES (libcxx
                            INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (
    libcxx
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/sk_libcxx.cpp
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_new.cpp
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_iostream.cpp)
||||||| bd1a0006
=======

# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

PROJECT(libcxx CXX ASM)

find_asm_source_files(ASM_SOURCE_FILES ${libcxx_SOURCE_DIR}/src)
aux_source_directory(${libcxx_SOURCE_DIR}/src cxx_src)

set(libcxx_src ${ASM_SOURCE_FILES} ${cxx_src})
add_library(${PROJECT_NAME} OBJECT ${libcxx_src})

target_include_libc_header_files(${PROJECT_NAME})
target_include_libcxx_header_files(${PROJECT_NAME})
>>>>>>> upstream/main
