# This file is a part of Simple-XX/SimpleKernel
# (https://github.com/Simple-XX/SimpleKernel).
#
# CMakeLists.txt for Simple-XX/SimpleKernel.

ADD_LIBRARY (apic INTERFACE)

TARGET_INCLUDE_DIRECTORIES (apic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)

TARGET_SOURCES (apic INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/apic.cpp)
