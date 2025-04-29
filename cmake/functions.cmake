# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# functions.cmake for Simple-XX/SimpleKernel.
# 辅助函数

# 生成 target 输出文件的 objdump -D, readelf -a, nm -a
# target: target 名
# 在 ${${target}_BINARY_DIR} 目录下生成 $<TARGET_FILE:${target}>.asm 文件
# 在 ${${target}_BINARY_DIR} 目录下生成 $<TARGET_FILE:${target}>.readelf 文件
# 在 ${${target}_BINARY_DIR} 目录下生成 $<TARGET_FILE:${target}>.sym 文件
FUNCTION(objdump_readelf_nm target)
    ADD_CUSTOM_COMMAND (
        TARGET ${target}
        VERBATIM POST_BUILD DEPENDS ${target}
        WORKING_DIRECTORY ${${target}_BINARY_DIR}
        COMMAND ${CMAKE_OBJDUMP} -D $<TARGET_FILE:${target}> >
                $<TARGET_FILE_DIR:${target}>/${target}.asm || exit 0
        COMMAND ${CMAKE_READELF} -a $<TARGET_FILE:${target}> >
                $<TARGET_FILE_DIR:${target}>/${target}.readelf || exit 0
        COMMAND ${CMAKE_NM} -a $<TARGET_FILE:${target}> >
                $<TARGET_FILE_DIR:${target}>/${target}.sym
        COMMAND ${CMAKE_OBJCOPY} -O binary $<TARGET_FILE:${target}>
                $<TARGET_FILE_DIR:${target}>/${target}.bin
        COMMENT "Generating symbol table, assembly, and readelf for ${target}")
    SET_DIRECTORY_PROPERTIES (
        PROPERTIES ADDITIONAL_MAKE_CLEAN_FILES
                   "$<TARGET_FILE_DIR:${target}>/${target}.asm;"
                   "$<TARGET_FILE_DIR:${target}>/${target}.readelf;"
                   "$<TARGET_FILE_DIR:${target}>/${target}.sym;")
ENDFUNCTION()

# 添加测试覆盖率 target
# DEPENDS 要生成的 targets
# SOURCE_DIR 源码路径
# BINARY_DIR 二进制文件路径
# EXCLUDE_DIR 要排除的目录
FUNCTION(add_coverage_target)
    # 解析参数
    SET (options)
    SET (one_value_keywords SOURCE_DIR BINARY_DIR)
    SET (multi_value_keywords DEPENDS EXCLUDE_DIR)
    CMAKE_PARSE_ARGUMENTS (ARG "${options}" "${one_value_keywords}"
                           "${multi_value_keywords}" ${ARGN})

    # 不检查的目录
    LIST (APPEND EXCLUDES --exclude)
    FOREACH(item ${ARG_EXCLUDE_DIR})
        LIST (APPEND EXCLUDES '${item}')
    ENDFOREACH()

    # 添加 target
    ADD_CUSTOM_TARGET (
        coverage
        COMMENT ""
        DEPENDS ${ARG_DEPENDS}
        COMMAND ${CMAKE_CTEST_COMMAND})
    # 在 coverage 执行完毕后生成报告
    ADD_CUSTOM_COMMAND (
        TARGET coverage
        COMMENT "Generating coverage report ..."
        POST_BUILD
        WORKING_DIRECTORY ${ARG_BINARY_DIR}
        COMMAND ${CMAKE_COMMAND} -E make_directory ${COVERAGE_OUTPUT_DIR}
        COMMAND
            ${LCOV_EXE} -c -o ${COVERAGE_OUTPUT_DIR}/coverage.info -d
            ${ARG_BINARY_DIR} -b ${ARG_SOURCE_DIR} --no-external ${EXCLUDES}
            --rc branch_coverage=1 --ignore-errors mismatch
        COMMAND ${GENHTML_EXE} ${COVERAGE_OUTPUT_DIR}/coverage.info -o
                ${COVERAGE_OUTPUT_DIR} --branch-coverage)
ENDFUNCTION()

# 添加在 qemu 中运行内核
# DEPENDS 依赖的 target
# QEMU_FLAGS qemu 参数
FUNCTION(add_run_target)
    # 解析参数
    SET (options)
    SET (multi_value_keywords DEPENDS QEMU_FLAGS)
    CMAKE_PARSE_ARGUMENTS (ARG "${options}" "${one_value_keywords}"
                           "${multi_value_keywords}" ${ARGN})

    # 添加 target
    ADD_CUSTOM_TARGET (
        run
        COMMENT "Run Simplekernel ..."
        DEPENDS ${ARG_DEPENDS}
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
        COMMAND ln -s -f ${CMAKE_BINARY_DIR}/bin/* /srv/tftp
        COMMAND qemu-system-${CMAKE_SYSTEM_PROCESSOR} ${ARG_QEMU_FLAGS})
    ADD_CUSTOM_TARGET (
        debug
        COMMENT "Debug Simplekernel ..."
        DEPENDS ${ARG_DEPENDS}
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
        COMMAND ln -s -f ${CMAKE_BINARY_DIR}/bin/* /srv/tftp
        COMMAND
            qemu-system-${CMAKE_SYSTEM_PROCESSOR} ${ARG_QEMU_FLAGS}
            # 等待 gdb 连接
            -S
            # 使用 1234 端口
            -gdb ${QEMU_GDB_PORT})
ENDFUNCTION()
