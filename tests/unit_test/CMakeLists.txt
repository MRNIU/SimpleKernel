# Copyright The SimpleKernel Contributors

# 设置项目名与版本
PROJECT (unit-test)

ENABLE_LANGUAGE (CXX)

ADD_EXECUTABLE (
    ${PROJECT_NAME} sk_libc_test.cpp sk_ctype_test.cpp sk_string_test.cpp
                    kernel_elf_test.cpp kernel_fdt_test.cpp spinlock_test.cpp)

TARGET_COMPILE_DEFINITIONS (${PROJECT_NAME} PRIVATE -Dsk_printf=printf
                                                    -DUNIT_TEST)

TARGET_INCLUDE_DIRECTORIES (
    ${PROJECT_NAME}
    PRIVATE ./ ${CMAKE_SOURCE_DIR}/src ${CMAKE_SOURCE_DIR}/src/include
            ${CMAKE_SOURCE_DIR}/src/arch ${CMAKE_SOURCE_DIR}/src/libc/include
            ${CMAKE_SOURCE_DIR}/src/libcxx/include)

TARGET_COMPILE_OPTIONS (${PROJECT_NAME} PRIVATE ${DEFAULT_TEST_COMPILE_OPTIONS})

TARGET_LINK_OPTIONS (${PROJECT_NAME} PRIVATE ${DEFAULT_TEST_LINK_OPTIONS})

TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE ${DEFAULT_TEST_LINK_LIB} dtc-lib)

GTEST_DISCOVER_TESTS (${PROJECT_NAME})
