# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

ADD_LIBRARY (plic INTERFACE)

TARGET_INCLUDE_DIRECTORIES (plic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (plic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/plic.cpp)
