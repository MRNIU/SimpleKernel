# Copyright The MPMCQueue Contributors

PROJECT (MPMCQueue_test)

ADD_EXECUTABLE (${PROJECT_NAME} test.cpp)

TARGET_COMPILE_OPTIONS (
    ${PROJECT_NAME}
    PRIVATE -g
            -ggdb
            -Wall
            -Wextra
            -pedantic
            --coverage
            -fprofile-update=atomic
            -O0)

TARGET_LINK_OPTIONS (${PROJECT_NAME} PRIVATE --coverage -fsanitize=leak
                     -fsanitize=address -fno-omit-frame-pointer)

# 添加要链接的库
TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE MPMCQueue gtest gmock gtest_main)

ADD_DEPENDENCIES (${PROJECT_NAME} MPMCQueue)

# 添加测试到CTest
INCLUDE (GoogleTest)
GTEST_DISCOVER_TESTS (${PROJECT_NAME})
