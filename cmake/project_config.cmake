# Copyright The SimpleKernel Contributors

# 在目标环境搜索 program
SET (CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
# 在目标环境搜索库文件
SET (CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
# 在目标环境搜索头文件
SET (CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)

MESSAGE (STATUS "CMAKE_SYSTEM_PROCESSOR is: ${CMAKE_SYSTEM_PROCESSOR}")
MESSAGE (STATUS "CMAKE_TOOLCHAIN_FILE is: ${CMAKE_TOOLCHAIN_FILE}")

# 生成项目配置头文件，传递给代码
CONFIGURE_FILE (${CMAKE_SOURCE_DIR}/tools/project_config.h.in
                ${CMAKE_SOURCE_DIR}/src/project_config.h @ONLY)
CONFIGURE_FILE (${CMAKE_SOURCE_DIR}/tools/.pre-commit-config.yaml.in
                ${CMAKE_SOURCE_DIR}/.pre-commit-config.yaml @ONLY)

SET_DIRECTORY_PROPERTIES (
    PROPERTIES ADDITIONAL_CLEAN_FILES
               "${CMAKE_BINARY_DIR}/bin ${CMAKE_BINARY_DIR}/lib")
