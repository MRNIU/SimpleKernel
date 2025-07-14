# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

ADD_LIBRARY (8259a INTERFACE)

TARGET_INCLUDE_DIRECTORIES (8259a INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (8259a INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/8259a.cpp)
