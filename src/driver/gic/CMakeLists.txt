# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

ADD_LIBRARY (gic INTERFACE)

TARGET_INCLUDE_DIRECTORIES (gic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (gic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/gic.cpp)
