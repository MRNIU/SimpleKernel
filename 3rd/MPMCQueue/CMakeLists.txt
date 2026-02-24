# Copyright The MPMCQueue Contributors

CMAKE_MINIMUM_REQUIRED (VERSION 3.20)

PROJECT (
    MPMCQueue
    DESCRIPTION
        "Multi-Producer Multi-Consumer Lock-Free Queue for Freestanding C++26"
    LANGUAGES CXX)

IF(${PROJECT_SOURCE_DIR} STREQUAL ${PROJECT_BINARY_DIR})
    MESSAGE (
        FATAL_ERROR
            "In-source builds not allowed. Please make a new directory (called a build directory) and run CMake from there."
    )
ENDIF()

ADD_LIBRARY (${PROJECT_NAME} INTERFACE)
TARGET_INCLUDE_DIRECTORIES (${PROJECT_NAME} INTERFACE include)

ADD_SUBDIRECTORY (${PROJECT_SOURCE_DIR}/test)
