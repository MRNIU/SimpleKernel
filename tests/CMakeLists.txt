# Copyright The SimpleKernel Contributors
# CMake build configuration

ADD_SUBDIRECTORY (system_test)
ADD_SUBDIRECTORY (integration_test)

# 仅在 host 环境下进行 ut
IF(CMAKE_SYSTEM_PROCESSOR STREQUAL CMAKE_HOST_SYSTEM_PROCESSOR)
    ENABLE_TESTING ()
    LIST (
        APPEND
        DEFAULT_TEST_COMPILE_OPTIONS
        -Wall
        -Wextra
        -pedantic
        --coverage
        -fprofile-update=atomic)

    LIST (APPEND DEFAULT_TEST_LINK_OPTIONS --coverage -fsanitize=leak
          -fsanitize=address -fno-omit-frame-pointer)

    LIST (APPEND DEFAULT_TEST_LINK_LIB gtest gmock gtest_main)

    ADD_SUBDIRECTORY (unit_test)

    ADD_COVERAGE_TARGET (
        DEPENDS
        unit_test
        SOURCE_DIR
        ${CMAKE_SOURCE_DIR}
        BINARY_DIR
        ${CMAKE_BINARY_DIR}
        EXCLUDE_DIR
        ${CMAKE_SOURCE_DIR}/3rd/*)

ENDIF()
