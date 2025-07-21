# Copyright The SimpleKernel Contributors
# CMake build configuration

# 设置项目名与版本
PROJECT (unit-test VERSION 0.0.1)

ENABLE_LANGUAGE (CXX)

ADD_EXECUTABLE (${PROJECT_NAME} kernel_elf_test.cpp kernel_fdt_test.cpp
                                spinlock_test.cpp)

TARGET_COMPILE_DEFINITIONS (${PROJECT_NAME} PRIVATE -Dsk_printf=printf
                                                    -DUNIT_TEST)

TARGET_INCLUDE_DIRECTORIES (
    ${PROJECT_NAME}
    PRIVATE ./ ${CMAKE_SOURCE_DIR}/src ${CMAKE_SOURCE_DIR}/src/include
            ${CMAKE_SOURCE_DIR}/src/arch)

TARGET_COMPILE_OPTIONS (${PROJECT_NAME} PRIVATE ${DEFAULT_TEST_COMPILE_OPTIONS})

TARGET_LINK_OPTIONS (${PROJECT_NAME} PRIVATE ${DEFAULT_TEST_LINK_OPTIONS})

TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE ${DEFAULT_TEST_LINK_LIB} dtc-lib
                                               cpu_io)

GTEST_DISCOVER_TESTS (${PROJECT_NAME})
