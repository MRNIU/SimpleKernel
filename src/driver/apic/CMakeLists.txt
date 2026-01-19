# Copyright The SimpleKernel Contributors

ADD_LIBRARY (apic INTERFACE)

TARGET_INCLUDE_DIRECTORIES (apic INTERFACE include)

TARGET_SOURCES (apic INTERFACE apic.cpp local_apic.cpp io_apic.cpp)
