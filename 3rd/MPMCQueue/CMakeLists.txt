# Copyright The MPMCQueue Contributors

cmake_minimum_required(VERSION 3.20)

project(MPMCQueue
    DESCRIPTION "Multi-Producer Multi-Consumer Lock-Free Queue for Freestanding C++26"
    LANGUAGES CXX
)

if (${PROJECT_SOURCE_DIR} STREQUAL ${PROJECT_BINARY_DIR})
    message(
            FATAL_ERROR
            "In-source builds not allowed. Please make a new directory (called a build directory) and run CMake from there."
    )
endif ()

ADD_LIBRARY (${PROJECT_NAME} INTERFACE)
TARGET_INCLUDE_DIRECTORIES (${PROJECT_NAME} INTERFACE include)

add_subdirectory(${PROJECT_SOURCE_DIR}/test)
