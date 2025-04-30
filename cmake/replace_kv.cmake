# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# replace_kv.cmake for Simple-XX/SimpleKernel.
# 动态替换文件中的键值对

# 键值替换函数
# 参数：
#   in_file  - 源文件路径
#   out_file  - 目标文件路径
#   [PAIRS] - 可变参数键值对列表 (KEY VALUE KEY VALUE...)
FUNCTION(replace_keys)
    # 参数校验
    LIST (LENGTH ARGN arg_count)
    IF(arg_count LESS 2)
        MESSAGE (
            FATAL_ERROR "Usage: replace_keys(in_file out_file [KEY VALUE]...)")
    ENDIF()

    # 提取前两个固定参数
    LIST (GET ARGN 0 in_file)
    LIST (GET ARGN 1 out_file)
    LIST (REMOVE_AT ARGN 0 1)

    # 检查键值对数量
    LIST (LENGTH ARGN pair_count)
    MATH (EXPR remainder "${pair_count} % 2")
    IF(NOT remainder EQUAL 0)
        MESSAGE (
            FATAL_ERROR
                "key-value pairs must be in pairs (KEY VALUE KEY VALUE...) ${ARGN}"
        )
    ENDIF()

    # 读取源文件
    IF(NOT EXISTS "${in_file}")
        MESSAGE (FATAL_ERROR "File not exist: ${in_file}")
    ENDIF()
    FILE (READ "${in_file}" TEXT)

    # 处理键值替换
    WHILE(ARGN)
        LIST (GET ARGN 0 key)
        LIST (GET ARGN 1 value)
        LIST (REMOVE_AT ARGN 0 1)

        # 安全替换模式
        STRING (REPLACE "@${key}@" "${value}" TEXT "${TEXT}")
        MESSAGE (STATUS "Replace KV: @${key}@ -> ${value}")
    ENDWHILE()

    # 确保目标目录存在
    GET_FILENAME_COMPONENT (target_dir "${out_file}" DIRECTORY)
    FILE (MAKE_DIRECTORY "${target_dir}")

    # 写入目标文件
    FILE (WRITE "${out_file}" "${TEXT}")
ENDFUNCTION()

# 脚本模式入口
IF(CMAKE_SCRIPT_MODE_FILE)
    IF(NOT DEFINED IN_FILE
       OR NOT DEFINED OUT_FILE
       OR NOT DEFINED KV_PAIRS)
        MESSAGE (
            FATAL_ERROR
                "Usage: cmake -D IN_FILE=<file> -D OUT_FILE=<file> -D KV_PAIRS=\"KEY1=VAL1;KEY2=VAL2\" -P replace_kv.cmake"
        )
    ENDIF()
    REPLACE_KEYS (${IN_FILE} ${OUT_FILE} ${KV_PAIRS})
ENDIF()
