/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_EXPECTED_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_EXPECTED_HPP_

#include <expected>

/// 内核错误码
enum class ErrorCode : uint64_t {
  kSuccess = 0,
  // ELF 相关错误 (0x100 - 0x1FF)
  kElfInvalidAddress = 0x100,
  kElfInvalidMagic = 0x101,
  kElfUnsupported32Bit = 0x102,
  kElfInvalidClass = 0x103,
  kElfSymtabNotFound = 0x104,
  kElfStrtabNotFound = 0x105,
  // FDT 相关错误 (0x200 - 0x2FF)
  kFdtInvalidAddress = 0x200,
  kFdtInvalidHeader = 0x201,
  kFdtNodeNotFound = 0x202,
  kFdtPropertyNotFound = 0x203,
  kFdtParseFailed = 0x204,
  kFdtInvalidPropertySize = 0x205,
  // SpinLock 相关错误 (0x300 - 0x3FF)
  kSpinLockRecursiveLock = 0x300,
  kSpinLockNotOwned = 0x301,
  // VirtualMemory 相关错误 (0x400 - 0x4FF)
  kVmAllocationFailed = 0x400,
  kVmMapFailed = 0x401,
  kVmUnmapFailed = 0x402,
  kVmInvalidPageTable = 0x403,
  kVmPageNotMapped = 0x404,
  // IPI 相关错误 (0x500 - 0x5FF)
  kIpiTargetOutOfRange = 0x500,
  kIpiSendFailed = 0x501,
  // APIC 相关错误 (0x600 - 0x6FF)
  kApicInitFailed = 0x600,
  kApicInvalidIrq = 0x601,
  kApicInvalidParameter = 0x602,
  kApicCodeCopyFailed = 0x603,
  kApicAddressNotAligned = 0x604,
  kApicAddressOutOfRange = 0x605,
  kApicIpiTimeout = 0x606,
  // 通用错误 (0xF00 - 0xFFF)
  kInvalidArgument = 0xF00,
  kOutOfMemory = 0xF01,
};

/// 获取错误码对应的错误信息
constexpr auto GetErrorMessage(ErrorCode code) -> const char* {
  switch (code) {
    case ErrorCode::kSuccess:
      return "Success";
    case ErrorCode::kElfInvalidAddress:
      return "Invalid ELF address";
    case ErrorCode::kElfInvalidMagic:
      return "Invalid ELF magic number";
    case ErrorCode::kElfUnsupported32Bit:
      return "32-bit ELF not supported";
    case ErrorCode::kElfInvalidClass:
      return "Invalid ELF class";
    case ErrorCode::kElfSymtabNotFound:
      return ".symtab section not found";
    case ErrorCode::kElfStrtabNotFound:
      return ".strtab section not found";
    case ErrorCode::kFdtInvalidAddress:
      return "Invalid FDT address";
    case ErrorCode::kFdtInvalidHeader:
      return "Invalid FDT header";
    case ErrorCode::kFdtNodeNotFound:
      return "FDT node not found";
    case ErrorCode::kFdtPropertyNotFound:
      return "FDT property not found";
    case ErrorCode::kFdtParseFailed:
      return "FDT parse failed";
    case ErrorCode::kFdtInvalidPropertySize:
      return "Invalid FDT property size";
    case ErrorCode::kSpinLockRecursiveLock:
      return "Recursive spinlock detected";
    case ErrorCode::kSpinLockNotOwned:
      return "Spinlock not owned by current core";
    case ErrorCode::kVmAllocationFailed:
      return "Virtual memory allocation failed";
    case ErrorCode::kVmMapFailed:
      return "Virtual memory mapping failed";
    case ErrorCode::kVmUnmapFailed:
      return "Virtual memory unmapping failed";
    case ErrorCode::kVmInvalidPageTable:
      return "Invalid page table";
    case ErrorCode::kVmPageNotMapped:
      return "Page not mapped";
    case ErrorCode::kIpiTargetOutOfRange:
      return "IPI target CPU mask out of range";
    case ErrorCode::kIpiSendFailed:
      return "IPI send failed";
    case ErrorCode::kApicInitFailed:
      return "APIC initialization failed";
    case ErrorCode::kApicInvalidIrq:
      return "Invalid IRQ number";
    case ErrorCode::kApicInvalidParameter:
      return "Invalid APIC parameter";
    case ErrorCode::kApicCodeCopyFailed:
      return "AP code copy verification failed";
    case ErrorCode::kApicAddressNotAligned:
      return "Address not aligned to required boundary";
    case ErrorCode::kApicAddressOutOfRange:
      return "Address out of valid range";
    case ErrorCode::kApicIpiTimeout:
      return "IPI delivery timeout";
    case ErrorCode::kInvalidArgument:
      return "Invalid argument";
    case ErrorCode::kOutOfMemory:
      return "Out of memory";
    default:
      return "Unknown error";
  }
}

/// 错误类型，用于 std::expected
struct Error {
  ErrorCode code;

  constexpr Error(ErrorCode c) : code(c) {}

  [[nodiscard]] constexpr auto message() const -> const char* {
    return GetErrorMessage(code);
  }
};

/// std::expected 别名模板
template <typename T>
using Expected = std::expected<T, Error>;

#endif /* SIMPLEKERNEL_SRC_INCLUDE_EXPECTED_HPP_ */
