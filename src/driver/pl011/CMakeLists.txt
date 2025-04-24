# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

ADD_LIBRARY (pl011 INTERFACE)

TARGET_INCLUDE_DIRECTORIES (pl011 INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (pl011 INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/pl011.cpp)
