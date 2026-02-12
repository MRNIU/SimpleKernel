/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_EXPECTED_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_EXPECTED_HPP_

#include <cstdint>
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
  // Task 相关错误 (0x700 - 0x7FF)
  kTaskNoCurrentTask = 0x700,
  kTaskPidAllocationFailed = 0x701,
  kTaskAllocationFailed = 0x702,
  kTaskInvalidCloneFlags = 0x703,
  kTaskPageTableCloneFailed = 0x704,
  kTaskKernelStackAllocationFailed = 0x705,
  kTaskNoChildFound = 0x706,
  kTaskInvalidPid = 0x707,
  // Device 相关错误 (0x800 - 0x8FF)
  kDeviceNotFound = 0x800,
  kDeviceAlreadyOpen = 0x801,
  kDeviceNotOpen = 0x802,
  kDeviceReadFailed = 0x803,
  kDeviceWriteFailed = 0x804,
  kDeviceIoctlFailed = 0x805,
  kDeviceMmapFailed = 0x806,
  kDeviceNotSupported = 0x807,
  kDeviceBusy = 0x808,
  kDevicePermissionDenied = 0x809,
  kDeviceInvalidOffset = 0x80A,
  kDeviceBlockUnaligned = 0x80B,
  kDeviceBlockOutOfRange = 0x80C,
  kDeviceFlushFailed = 0x80D,
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
    case ErrorCode::kTaskNoCurrentTask:
      return "No current task";
    case ErrorCode::kTaskPidAllocationFailed:
      return "PID allocation failed";
    case ErrorCode::kTaskAllocationFailed:
      return "Task allocation failed";
    case ErrorCode::kTaskInvalidCloneFlags:
      return "Invalid clone flags";
    case ErrorCode::kTaskPageTableCloneFailed:
      return "Page table clone failed";
    case ErrorCode::kTaskKernelStackAllocationFailed:
      return "Kernel stack allocation failed";
    case ErrorCode::kTaskNoChildFound:
      return "No child process found";
    case ErrorCode::kTaskInvalidPid:
      return "Invalid PID";
    case ErrorCode::kDeviceNotFound:
      return "Device not found";
    case ErrorCode::kDeviceAlreadyOpen:
      return "Device already open";
    case ErrorCode::kDeviceNotOpen:
      return "Device not open";
    case ErrorCode::kDeviceReadFailed:
      return "Device read failed";
    case ErrorCode::kDeviceWriteFailed:
      return "Device write failed";
    case ErrorCode::kDeviceIoctlFailed:
      return "Device ioctl failed";
    case ErrorCode::kDeviceMmapFailed:
      return "Device mmap failed";
    case ErrorCode::kDeviceNotSupported:
      return "Operation not supported by device";
    case ErrorCode::kDeviceBusy:
      return "Device busy";
    case ErrorCode::kDevicePermissionDenied:
      return "Device permission denied";
    case ErrorCode::kDeviceInvalidOffset:
      return "Invalid device offset";
    case ErrorCode::kDeviceBlockUnaligned:
      return "Block access not aligned to block size";
    case ErrorCode::kDeviceBlockOutOfRange:
      return "Block number out of device range";
    case ErrorCode::kDeviceFlushFailed:
      return "Device flush failed";
    case ErrorCode::kInvalidArgument:
      return "Invalid argument";
    case ErrorCode::kOutOfMemory:
      return "Out of memory";
    default:
      return "Unknown error";
  }
}

/// 错误类型，用于 std::expected
/// @todo 还需要传递一个具体错误信息的字符串
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
