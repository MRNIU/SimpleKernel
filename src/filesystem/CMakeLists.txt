# Copyright The SimpleKernel Contributors

ADD_LIBRARY (filesystem STATIC vfs/vfs.cpp vfs/mount.cpp file_descriptor.cpp)

TARGET_INCLUDE_DIRECTORIES (
    filesystem
    PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include
           ${CMAKE_CURRENT_SOURCE_DIR}/vfs/include
           ${CMAKE_SOURCE_DIR}/src/include
           ${CMAKE_SOURCE_DIR}/src/libcxx/include
           ${CMAKE_SOURCE_DIR}/src/libc/include
           ${CMAKE_SOURCE_DIR}/3rd/cpu_io/include)

# 添加 ramfs 子目录
ADD_SUBDIRECTORY (ramfs)

TARGET_LINK_LIBRARIES (filesystem PUBLIC ramfs)
