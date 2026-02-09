# Copyright The MPMCQueue Contributors

project(
        MPMCQueue_test
)

add_executable(${PROJECT_NAME}
        test.cpp
)

target_compile_options(${PROJECT_NAME} PRIVATE
        -g
        -ggdb
        -Wall
        -Wextra
        -pedantic
        --coverage
        -fprofile-update=atomic
        -O0
)

target_link_options(${PROJECT_NAME} PRIVATE
        --coverage
        -fsanitize=leak
        -fsanitize=address
        -fno-omit-frame-pointer
)

# 添加要链接的库
target_link_libraries(${PROJECT_NAME} PRIVATE
        MPMCQueue
        gtest
        gmock
        gtest_main
)

add_dependencies(${PROJECT_NAME}
        MPMCQueue
)

# 添加测试到CTest
include(GoogleTest)
gtest_discover_tests(${PROJECT_NAME})
