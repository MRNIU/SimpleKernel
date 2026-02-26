# Copyright The SimpleKernel Contributors

ADD_LIBRARY (fatfs INTERFACE)

TARGET_INCLUDE_DIRECTORIES (fatfs INTERFACE include)

TARGET_SOURCES (fatfs INTERFACE)

# Link with the upstream fatfs library
# TARGET_LINK_LIBRARIES (fatfs_kernel PUBLIC fatfs)
