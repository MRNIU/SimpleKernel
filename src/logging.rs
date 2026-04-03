//! 内核日志后端——基于 `log` crate，通过 `SpinLockIrq` 保护串口输出。

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use sync::SpinLockIrq;

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_RED: &str = "\x1b[31m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_GRAY: &str = "\x1b[90m";

static CONSOLE_LOCK: SpinLockIrq<()> = SpinLockIrq::new((), "console", sync::lock_level::CONSOLE);
static LOG_SEQ: AtomicU64 = AtomicU64::new(0);
static LOGGER_INIT: AtomicBool = AtomicBool::new(false);
static LOGGER: KernelLogger = KernelLogger;

fn put_str(s: &str) {
    #[cfg(target_os = "none")]
    {
        use crate::arch::ArchOps;
        crate::arch::Arch::console_write(s);
    }
    #[cfg(not(target_os = "none"))]
    {
        let _ = s;
    }
}

fn level_color(level: log::Level) -> &'static str {
    match level {
        log::Level::Trace => ANSI_GRAY,
        log::Level::Debug => ANSI_GREEN,
        log::Level::Info => ANSI_CYAN,
        log::Level::Warn => ANSI_YELLOW,
        log::Level::Error => ANSI_RED,
    }
}

fn level_label(level: log::Level) -> &'static str {
    match level {
        log::Level::Trace => "TRACE",
        log::Level::Debug => "DEBUG",
        log::Level::Info => "INFO ",
        log::Level::Warn => "WARN ",
        log::Level::Error => "ERROR",
    }
}

struct KernelLogger;

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let seq = LOG_SEQ.fetch_add(1, Ordering::Relaxed);
        let core_id = per_cpu::current_core_id();
        let level = record.level();

        let mut buf = heapless::String::<{ config::LOG_MSG_BUF_SIZE }>::new();
        let truncated = write!(&mut buf, "{}", record.args()).is_err();

        let mut hdr = heapless::String::<{ config::LOG_HDR_BUF_SIZE }>::new();
        let _ = write!(
            &mut hdr,
            "{}[{}][{} {}] ",
            level_color(level),
            seq,
            core_id,
            level_label(level)
        );

        let _guard = CONSOLE_LOCK.lock();
        put_str(hdr.as_str());
        put_str(buf.as_str());
        if truncated {
            put_str("...[truncated]");
        }
        put_str(ANSI_RESET);
        put_str("\n");
    }

    fn flush(&self) {}
}

pub fn init() {
    if LOGGER_INIT.swap(true, Ordering::AcqRel) {
        return;
    }
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(config::DEFAULT_LOG_LEVEL);
    }
}

pub fn flush() {}

pub fn raw_put(msg: &str) {
    put_str(msg);
}
