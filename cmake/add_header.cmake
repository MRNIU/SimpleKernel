
# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# add_header.cmake for Simple-XX/SimpleKernel.
# 将头文件路径添加到 _target 的搜索路径中

function(add_header_project _target)
    target_include_directories(${_target} PRIVATE
            ${CMAKE_SOURCE_DIR}/src)
endfunction()
