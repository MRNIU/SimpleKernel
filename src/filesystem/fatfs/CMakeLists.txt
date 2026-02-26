# Copyright The SimpleKernel Contributors

ADD_LIBRARY (fatfs INTERFACE)

TARGET_INCLUDE_DIRECTORIES (
    fatfs INTERFACE include ${CMAKE_SOURCE_DIR}/3rd/fatfs/source)

TARGET_SOURCES (fatfs INTERFACE diskio.cpp fatfs_filesystem.cpp)

TARGET_LINK_LIBRARIES (fatfs INTERFACE fatfs_lib)
