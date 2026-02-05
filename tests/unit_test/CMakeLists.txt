# Copyright The SimpleKernel Contributors
# CMake build configuration

# 设置项目名与版本
PROJECT (unit-test)

ENABLE_LANGUAGE (CXX)

ADD_EXECUTABLE (
    ${PROJECT_NAME}
    sk_libc_test.cpp
    sk_ctype_test.cpp
    sk_string_test.cpp
    kernel_elf_test.cpp
    kernel_fdt_test.cpp
    spinlock_test.cpp
    virtual_memory_test.cpp
    sk_list_test.cpp
    sk_queue_test.cpp
    sk_vector_test.cpp
    sk_priority_queue_test.cpp
    sk_rb_tree_test.cpp
    sk_set_test.cpp
    sk_unordered_map_test.cpp
    task/example_environment_test.cpp
    task/task_scheduling_example_test.cpp
    task/fixtures/task_test_harness.cpp
    mocks/arch.cpp
    mocks/test_environment_state.cpp
    ${CMAKE_SOURCE_DIR}/src/syscall.cpp
    ${CMAKE_SOURCE_DIR}/src/task/block.cpp
    ${CMAKE_SOURCE_DIR}/src/task/clone.cpp
    ${CMAKE_SOURCE_DIR}/src/task/exit.cpp
    ${CMAKE_SOURCE_DIR}/src/task/schedule.cpp
    ${CMAKE_SOURCE_DIR}/src/task/sleep.cpp
    ${CMAKE_SOURCE_DIR}/src/task/task_control_block.cpp
    ${CMAKE_SOURCE_DIR}/src/task/task_manager.cpp
    ${CMAKE_SOURCE_DIR}/src/task/tick_update.cpp
    ${CMAKE_SOURCE_DIR}/src/task/wait.cpp
    ${CMAKE_SOURCE_DIR}/src/task/wakeup.cpp)

TARGET_COMPILE_DEFINITIONS (
    ${PROJECT_NAME}
    PRIVATE -Dsk_printf=printf
            -DUNIT_TEST
            -DSIMPLEKERNEL_SRC_LIBCXX_INCLUDE_SK_NEW_
            -DSIMPLEKERNEL_MAX_CORE_COUNT=4
            -DSIMPLEKERNEL_PER_CPU_ALIGN_SIZE=4096
            -DSIMPLEKERNEL_TICK=1000)

TARGET_INCLUDE_DIRECTORIES (
    ${PROJECT_NAME}
    PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}
            ${CMAKE_CURRENT_SOURCE_DIR}/mocks
            ${CMAKE_CURRENT_SOURCE_DIR}/task/fixtures
            ${CMAKE_SOURCE_DIR}/src
            ${CMAKE_SOURCE_DIR}/src/include
            ${CMAKE_SOURCE_DIR}/src/arch
            ${CMAKE_SOURCE_DIR}/src/libc/include
            ${CMAKE_SOURCE_DIR}/src/libcxx/include
            ${CMAKE_SOURCE_DIR}/src/task/include)

TARGET_COMPILE_OPTIONS (${PROJECT_NAME} PRIVATE ${DEFAULT_TEST_COMPILE_OPTIONS})

TARGET_LINK_OPTIONS (${PROJECT_NAME} PRIVATE ${DEFAULT_TEST_LINK_OPTIONS})

TARGET_LINK_LIBRARIES (${PROJECT_NAME} PRIVATE ${DEFAULT_TEST_LINK_LIB} dtc-lib
                                               bmalloc MPMCQueue task)

GTEST_DISCOVER_TESTS (${PROJECT_NAME})
