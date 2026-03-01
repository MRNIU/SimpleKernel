# Copyright The SimpleKernel Contributors

ADD_SUBDIRECTORY (ns16550a)
ADD_SUBDIRECTORY (pl011)
ADD_SUBDIRECTORY (acpi)
ADD_SUBDIRECTORY (virtio)

ADD_LIBRARY (device STATIC device.cpp)

TARGET_INCLUDE_DIRECTORIES (
    device
    PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include
    PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}
            ${CMAKE_SOURCE_DIR}/src/include
            ${CMAKE_SOURCE_DIR}/src/libc/include
            ${CMAKE_SOURCE_DIR}/src/libcxx/include
            ${CMAKE_SOURCE_DIR}/src/memory/include
            ${CMAKE_SOURCE_DIR}/src/filesystem/vfs/include)

TARGET_LINK_LIBRARIES (
    device
    PRIVATE compile_options
            compile_definitions
            kernel_compile_options
            kernel_compile_definitions
            nanoprintf-lib
            cpu_io
            dtc-lib
            etl::etl
            ns16550a_driver
            pl011_driver
            acpi_driver
            virtio_driver)
