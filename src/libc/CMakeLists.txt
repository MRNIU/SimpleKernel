# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

ADD_LIBRARY (libc INTERFACE)

TARGET_INCLUDE_DIRECTORIES (libc INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (
    libc
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/sk_libc.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_string.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_stdio.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_ctype.c)
