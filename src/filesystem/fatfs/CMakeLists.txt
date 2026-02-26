# Copyright The SimpleKernel Contributors

ADD_LIBRARY (fatfs INTERFACE)

TARGET_INCLUDE_DIRECTORIES (fatfs INTERFACE include)

TARGET_SOURCES (fatfs INTERFACE diskio.cpp fatfs_filesystem.cpp)
