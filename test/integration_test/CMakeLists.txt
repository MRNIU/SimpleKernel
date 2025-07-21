# Copyright The SimpleKernel Contributors
# CMake build configuration

# 设置项目名与版本
PROJECT (integration-test VERSION 0.0.1)

ADD_EXECUTABLE (integration_test example.cpp)

TARGET_COMPILE_OPTIONS (integration_test
                        PRIVATE ${DEFAULT_TEST_COMPILE_OPTIONS})

TARGET_LINK_OPTIONS (integration_test PRIVATE ${DEFAULT_TEST_LINK_OPTIONS})

TARGET_LINK_LIBRARIES (integration_test PRIVATE ${DEFAULT_TEST_LINK_LIB})

GTEST_DISCOVER_TESTS (integration_test)
