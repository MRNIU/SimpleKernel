# Copyright The SimpleKernel Contributors

ADD_LIBRARY (ramfs STATIC ramfs.cpp)

TARGET_INCLUDE_DIRECTORIES (
    ramfs
    PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include
           ${CMAKE_SOURCE_DIR}/src/filesystem/include
           ${CMAKE_SOURCE_DIR}/src/filesystem/vfs/include
           ${CMAKE_SOURCE_DIR}/src/include
           ${CMAKE_SOURCE_DIR}/src/libcxx/include
           ${CMAKE_SOURCE_DIR}/src/libc/include
           ${CMAKE_SOURCE_DIR}/3rd/cpu_io/include)

# We cannot recursively link ramfs to filesystem and filesystem to ramfs.
# But wait, ramfs needs VFS headers.
