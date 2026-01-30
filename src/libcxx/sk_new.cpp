/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstddef>
#include <new>

[[nodiscard]] void* operator new(size_t) { return nullptr; }

[[nodiscard]] void* operator new[](size_t) { return nullptr; }

[[nodiscard]] void* operator new(size_t, size_t) noexcept { return nullptr; }

[[nodiscard]] void* operator new[](size_t, size_t) noexcept { return nullptr; }

void operator delete(void*) noexcept {}

void operator delete(void*, size_t) noexcept {}

void operator delete[](void*) noexcept {}

void operator delete[](void*, size_t) noexcept {}

void operator delete(void*, size_t, size_t) noexcept {}

void operator delete[](void*, size_t, size_t) noexcept {}
