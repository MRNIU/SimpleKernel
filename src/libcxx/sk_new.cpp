/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_new"

#include <cstddef>

void* operator new(size_t) noexcept { return nullptr; }

void* operator new[](size_t) noexcept { return nullptr; }

void* operator new(size_t, size_t) noexcept { return nullptr; }

void* operator new[](size_t, size_t) noexcept { return nullptr; }

void operator delete(void*) { ; }

void operator delete(void*, size_t) { ; }

void operator delete[](void*) { ; }

void operator delete[](void*, size_t) { ; }
