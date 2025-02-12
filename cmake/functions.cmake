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
        COMMENT "Generating symbol table, assembly, and readelf for ${target}")
    SET_DIRECTORY_PROPERTIES (
        PROPERTIES ADDITIONAL_MAKE_CLEAN_FILES
                   "$<TARGET_FILE_DIR:${target}>/${target}.asm;"
                   "$<TARGET_FILE_DIR:${target}>/${target}.readelf;"
                   "$<TARGET_FILE_DIR:${target}>/${target}.sym;")
ENDFUNCTION()

# 将 elf 转换为 efi
# efi: 输出的 efi 文件名
# 在 ${${target}_BINARY_DIR} 目录下生成 ${efi} 文件
FUNCTION(elf2efi target efi)
    ADD_CUSTOM_COMMAND (
        TARGET ${target}
        COMMENT "Convert $<TARGET_FILE:${target}> to efi ..."
        POST_BUILD DEPENDS ${target}
        WORKING_DIRECTORY ${${target}_BINARY_DIR}
        COMMAND
            ${CMAKE_OBJCOPY} $<TARGET_FILE:${target}> ${efi} -S -R .comment -R
            .note.gnu.build-id -R .gnu.hash -R .dynsym
            --target=efi-app-${CMAKE_SYSTEM_PROCESSOR} --subsystem=10)
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

# 添加运行 qemu target
# NAME 生成的 target 前缀
# TARGET 目标架构
# WORKING_DIRECTORY 工作目录
# BOOT boot 文件路径
# KERNEL kernel 文件路径
# DEPENDS 依赖的 target
# QEMU_FLAGS qemu 参数
FUNCTION(add_run_target)
    # 解析参数
    SET (options)
    SET (one_value_keywords NAME TARGET WORKING_DIRECTORY BOOT KERNEL)
    SET (multi_value_keywords DEPENDS QEMU_FLAGS)
    CMAKE_PARSE_ARGUMENTS (ARG "${options}" "${one_value_keywords}"
                           "${multi_value_keywords}" ${ARGN})

    LIST (
        APPEND
        commands
        COMMAND
        ${CMAKE_COMMAND}
        -E
        copy
        ${ARG_KERNEL}
        image/)
    IF(${ARG_TARGET} STREQUAL "x86_64")
        GET_FILENAME_COMPONENT (BOOT_FILE_NAME ${ARG_BOOT} NAME)
        CONFIGURE_FILE (${CMAKE_SOURCE_DIR}/tools/startup.nsh.in
                        image/startup.nsh @ONLY)
        LIST (
            APPEND
            commands
            COMMAND
            ${CMAKE_COMMAND}
            -E
            copy
            ${ARG_BOOT}
            image/)
    ELSEIF(${ARG_TARGET} STREQUAL "riscv64")
        GET_FILENAME_COMPONENT (BOOT_FILE_NAME ${ARG_BOOT} NAME)
        CONFIGURE_FILE (${CMAKE_SOURCE_DIR}/tools/startup.nsh.in
                        image/startup.nsh @ONLY)
        LIST (
            APPEND
            commands
            COMMAND
            ${CMAKE_COMMAND}
            -E
            copy
            ${ARG_BOOT}
            image/)
    ELSEIF(${ARG_TARGET} STREQUAL "aarch64")
        GET_FILENAME_COMPONENT (BOOT_FILE_NAME ${ARG_BOOT} NAME)
        CONFIGURE_FILE (${CMAKE_SOURCE_DIR}/tools/startup.nsh.in
                        image/startup.nsh @ONLY)
        LIST (
            APPEND
            commands
            COMMAND
            ${CMAKE_COMMAND}
            -E
            copy
            ${ARG_BOOT}
            image/)
    ENDIF()

    # 添加 target
    ADD_CUSTOM_TARGET (
        ${ARG_NAME}run
        COMMENT "Run ${ARG_NAME} ..."
        DEPENDS ${ARG_DEPENDS}
        WORKING_DIRECTORY ${ARG_WORKING_DIRECTORY}
        COMMAND ${CMAKE_COMMAND} -E make_directory image/ ${commands}
        COMMAND qemu-system-${ARG_TARGET} ${ARG_QEMU_FLAGS})
    ADD_CUSTOM_TARGET (
        ${ARG_NAME}debug
        COMMENT "Run ${ARG_NAME} ..."
        DEPENDS ${ARG_DEPENDS}
        WORKING_DIRECTORY ${ARG_WORKING_DIRECTORY}
        COMMAND ${CMAKE_COMMAND} -E make_directory image/ ${commands}
        COMMAND
            qemu-system-${ARG_TARGET} ${ARG_QEMU_FLAGS}
            # 等待 gdb 连接
            -S
            # 使用 1234 端口
            -gdb ${QEMU_GDB_PORT})
ENDFUNCTION()

# 定义宏处理参数化配置
FUNCTION(add_soc_term_target TARGET_NAME PORT_NUM)
    ADD_CUSTOM_TARGET (
        ${TARGET_NAME} COMMAND python3 ${optee_build_SOURCE_DIR}/soc_term.py
                               ${PORT_NUM})
ENDFUNCTION()
