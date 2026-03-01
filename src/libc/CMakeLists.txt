# Copyright The SimpleKernel Contributors

ADD_LIBRARY (libc INTERFACE)

TARGET_INCLUDE_DIRECTORIES (libc INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (
    libc
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/sk_stdlib.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_string.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_ctype.c)
