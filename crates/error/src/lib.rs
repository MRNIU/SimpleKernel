#![cfg_attr(not(test), no_std)]

use core::fmt;

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ElfInvalidAddress = 0x100,
    ElfInvalidMagic = 0x101,
    ElfUnsupported32Bit = 0x102,
    ElfInvalidClass = 0x103,
    ElfSymtabNotFound = 0x104,
    ElfStrtabNotFound = 0x105,
    FdtInvalidAddress = 0x200,
    FdtInvalidHeader = 0x201,
    FdtNodeNotFound = 0x202,
    FdtPropertyNotFound = 0x203,
    FdtParseFailed = 0x204,
    FdtInvalidPropertySize = 0x205,
    TaskNoCurrentTask = 0x700,
    TaskPidAllocationFailed = 0x701,
    TaskAllocationFailed = 0x702,
    TaskInvalidCloneFlags = 0x703,
    TaskPageTableCloneFailed = 0x704,
    TaskKernelStackAllocationFailed = 0x705,
    TaskNoChildFound = 0x706,
    TaskInvalidPid = 0x707,
    SignalInvalidNumber = 0xC00,
    SignalInvalidPid = 0xC01,
    SignalPermissionDenied = 0xC02,
    SignalUncatchable = 0xC03,
    SignalTaskNotFound = 0xC04,
    InvalidArgument = 0xF00,
    OutOfMemory = 0xF01,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for ErrorCode {}

pub type KResult<T> = Result<T, ErrorCode>;
