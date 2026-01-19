# Copyright The SimpleKernel Contributors

ADD_LIBRARY (task INTERFACE)

TARGET_INCLUDE_DIRECTORIES (task INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (
    task INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/task_control_block.cpp
                   ${CMAKE_CURRENT_SOURCE_DIR}/task_manager.cpp)
