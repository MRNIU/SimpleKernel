# Copyright The SimpleKernel Contributors

ADD_LIBRARY (ramfs STATIC ramfs.cpp)

TARGET_INCLUDE_DIRECTORIES (
    ramfs
    PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include
           ${CMAKE_SOURCE_DIR}/src/vfs/include
           ${CMAKE_SOURCE_DIR}/src/include
           ${CMAKE_SOURCE_DIR}/src/libcxx/include
           ${CMAKE_SOURCE_DIR}/src/libc/include
           ${CMAKE_SOURCE_DIR}/src/libc/include
           ${CMAKE_SOURCE_DIR}/3rd/cpu_io/include)

TARGET_LINK_LIBRARIES (ramfs PUBLIC vfs)
