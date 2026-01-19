# Copyright The SimpleKernel Contributors

ADD_LIBRARY (libcxx INTERFACE)

TARGET_INCLUDE_DIRECTORIES (libcxx INTERFACE include)

TARGET_SOURCES (libcxx INTERFACE sk_libcxx.cpp sk_new.cpp sk_iostream.cpp)
