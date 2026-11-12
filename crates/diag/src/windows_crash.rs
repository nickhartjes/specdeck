//! Native crashes on Windows (`diagnostics-launch-and-crashes` design decision 3): an exception
//! nothing handles, such as an access violation or a stack overflow, ends the process without a
//! panic. An unhandled-exception filter writes `app.crashed` straight to today's log file first,
//! with the exception's code and the file name of the module it happened in, then lets Windows
//! go on as before.
//!
//! The filter runs in a process that is falling over, for a stack overflow on a thread with
//! little stack left: it allocates nothing, takes no locks and formats into a buffer on its
//! stack.
//!
//! The line's formatting is plain Rust, compiled and tested everywhere; only the filter is
//! Windows'.
#![cfg_attr(not(windows), allow(dead_code))]

/// The longest line the filter writes.
const LINE: usize = 512;
/// The longest module name kept.
const MODULE: usize = 64;

/// A time in UTC, as Windows gives it without allocating.
#[derive(Clone, Copy)]
struct Utc {
    year: u16,
    month: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    millis: u16,
}

/// Bytes into a fixed buffer, dropping what does not fit.
struct Cursor<'a> {
    buffer: &'a mut [u8],
    len: usize,
}

impl Cursor<'_> {
    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.len < self.buffer.len() {
                self.buffer[self.len] = byte;
                self.len += 1;
            }
        }
    }

    /// `value` in decimal, padded with zeros to `width` digits.
    fn decimal(&mut self, value: u64, width: usize) {
        let mut digits = [0u8; 20];
        let mut at = digits.len();
        let mut rest = value;
        loop {
            at -= 1;
            digits[at] = b'0' + (rest % 10) as u8;
            rest /= 10;
            if rest == 0 && digits.len() - at >= width {
                break;
            }
        }
        self.bytes(&digits[at..]);
    }

    /// `value` as `0x…` in upper-case hex, at least `width` digits.
    fn hex(&mut self, value: u64, width: usize) {
        let mut digits = [0u8; 16];
        let mut at = digits.len();
        let mut rest = value;
        loop {
            at -= 1;
            digits[at] = b"0123456789ABCDEF"[(rest & 0xF) as usize];
            rest >>= 4;
            if rest == 0 && digits.len() - at >= width {
                break;
            }
        }
        self.bytes(b"0x");
        self.bytes(&digits[at..]);
    }
}

/// The file name at the end of a module's path, as UTF-16 from Windows: letters, digits and
/// `.`, `_`, `-` only, at most [`MODULE`] of them, so the line needs no escaping and holds no
/// folder. Returns how many bytes it wrote into `out`.
fn module_name(path: &[u16], out: &mut [u8; MODULE]) -> usize {
    let start = path
        .iter()
        .rposition(|&unit| unit == u16::from(b'\\') || unit == u16::from(b'/'))
        .map_or(0, |at| at + 1);
    let mut len = 0;
    for &unit in &path[start..] {
        let kept = u8::try_from(unit)
            .ok()
            .filter(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
        if let Some(byte) = kept {
            if len == MODULE {
                break;
            }
            out[len] = byte;
            len += 1;
        }
    }
    len
}

/// The crash's line, as the log's other lines: one JSON object with its keys in order, and a
/// newline. Returns its length in `buffer`.
fn crash_line(
    buffer: &mut [u8; LINE],
    time: Utc,
    code: u32,
    module: &[u8],
    offset: u64,
    thread_id: u32,
) -> usize {
    let mut out = Cursor { buffer, len: 0 };
    out.bytes(b"{\"code\":\"");
    out.hex(u64::from(code), 8);
    out.bytes(b"\",\"event\":\"app.crashed\",\"level\":\"ERROR\",\"module\":\"");
    out.bytes(module);
    out.bytes(b"\",\"offset\":\"");
    out.hex(offset, 1);
    out.bytes(b"\",\"thread_id\":");
    out.decimal(u64::from(thread_id), 1);
    out.bytes(b",\"ts\":\"");
    out.decimal(u64::from(time.year), 4);
    out.bytes(b"-");
    out.decimal(u64::from(time.month), 2);
    out.bytes(b"-");
    out.decimal(u64::from(time.day), 2);
    out.bytes(b"T");
    out.decimal(u64::from(time.hour), 2);
    out.bytes(b":");
    out.decimal(u64::from(time.minute), 2);
    out.bytes(b":");
    out.decimal(u64::from(time.second), 2);
    out.bytes(b".");
    out.decimal(u64::from(time.millis), 3);
    out.bytes(b"Z\"}\n");
    out.len
}

#[cfg(windows)]
pub use imp::{install_exception_filter, reopen_if_new_day};

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

    use time::OffsetDateTime;
    use windows_sys::Win32::Foundation::{HMODULE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_APPEND_DATA, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_ALWAYS, WriteFile,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        EXCEPTION_POINTERS, LPTOP_LEVEL_EXCEPTION_FILTER, SetUnhandledExceptionFilter,
    };
    use windows_sys::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        GetModuleFileNameW, GetModuleHandleExW,
    };
    use windows_sys::Win32::System::SystemInformation::GetSystemTime;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;

    use super::{LINE, MODULE, Utc, crash_line, module_name};
    use crate::APP_LOG;

    /// The open handle to today's app log, as an integer; 0 before there is one.
    static FILE: AtomicUsize = AtomicUsize::new(0);
    /// The day [`FILE`] is for, as days since 1970.
    static DAY: AtomicI32 = AtomicI32::new(i32::MIN);
    /// The log folder, for the next day's file.
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    /// The filter that was there before ours, as an integer; 0 for none.
    static PREVIOUS: AtomicUsize = AtomicUsize::new(0);
    /// Set by the first crash: one line, also when the filter runs again.
    static CRASHED: AtomicBool = AtomicBool::new(false);

    /// Opens today's app log in `dir` for the filter and sets the filter, keeping the one before
    /// it. Called once, after everything else that could set one.
    pub fn install_exception_filter(dir: &Path) {
        if DIR.set(dir.to_owned()).is_err() {
            return;
        }
        reopen_if_new_day();
        // SAFETY: `on_crash` has the signature Windows calls it with, and lives as long as the
        // process.
        let previous = unsafe { SetUnhandledExceptionFilter(Some(on_crash)) };
        PREVIOUS.store(previous.map_or(0, |filter| filter as usize), Ordering::SeqCst);
    }

    /// Opens the new day's file once the date (UTC) changed, as the log's own files do; called
    /// every few seconds. The previous day's handle stays open: the filter may hold it.
    pub fn reopen_if_new_day() {
        let Some(dir) = DIR.get() else { return };
        let today = OffsetDateTime::now_utc().date();
        let day = today.to_julian_day();
        if DAY.load(Ordering::SeqCst) == day {
            return;
        }
        let path = dir.join(format!("{APP_LOG}.{today}"));
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `wide` is a NUL-terminated path that outlives the call.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                FILE_APPEND_DATA,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            FILE.store(handle as usize, Ordering::SeqCst);
            DAY.store(day, Ordering::SeqCst);
        }
    }

    unsafe extern "system" fn on_crash(info: *const EXCEPTION_POINTERS) -> i32 {
        if !CRASHED.swap(true, Ordering::SeqCst) {
            // SAFETY: Windows passes the exception's pointers, valid during this call.
            unsafe { record(info) };
        }
        let previous = PREVIOUS.load(Ordering::SeqCst);
        if previous != 0 {
            // SAFETY: the value came from `SetUnhandledExceptionFilter`, a filter of this type.
            let previous: LPTOP_LEVEL_EXCEPTION_FILTER = unsafe { std::mem::transmute(previous) };
            if let Some(previous) = previous {
                // SAFETY: as Windows would have called it without ours.
                return unsafe { previous(info) };
            }
        }
        // EXCEPTION_CONTINUE_SEARCH: Windows Error Reporting goes on as before.
        0
    }

    /// Writes the crash's line to today's file.
    unsafe fn record(info: *const EXCEPTION_POINTERS) {
        let file = FILE.load(Ordering::SeqCst);
        if file == 0 || info.is_null() {
            return;
        }
        // SAFETY: Windows passes a valid exception record with the pointers.
        let (code, address) = unsafe {
            let record = (*info).ExceptionRecord;
            if record.is_null() {
                return;
            }
            ((*record).ExceptionCode as u32, (*record).ExceptionAddress as usize)
        };
        let mut module: HMODULE = std::ptr::null_mut();
        let mut path = [0u16; 1024];
        // SAFETY: the address is only looked up, not read; the buffer's length goes with it.
        let found = unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                address as *const u16,
                &mut module,
            ) != 0
        };
        let mut name = [0u8; MODULE];
        let (name_len, offset) = if found {
            // SAFETY: as above.
            let len = unsafe { GetModuleFileNameW(module, path.as_mut_ptr(), path.len() as u32) };
            let len = (len as usize).min(path.len());
            (module_name(&path[..len], &mut name), address.wrapping_sub(module as usize))
        } else {
            (0, address)
        };
        // SAFETY: plain calls that fill a struct or return a number.
        let (time, thread_id) = unsafe {
            let mut now = std::mem::zeroed();
            GetSystemTime(&mut now);
            (now, GetCurrentThreadId())
        };
        let time = Utc {
            year: time.wYear,
            month: time.wMonth,
            day: time.wDay,
            hour: time.wHour,
            minute: time.wMinute,
            second: time.wSecond,
            millis: time.wMilliseconds,
        };
        let mut line = [0u8; LINE];
        let len = crash_line(&mut line, time, code, &name[..name_len], offset as u64, thread_id);
        let mut written = 0;
        // SAFETY: the handle was opened for appending and is never closed; the buffer's length
        // goes with it.
        unsafe {
            WriteFile(file as _, line.as_ptr(), len as u32, &mut written, std::ptr::null_mut());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn a_crash_line_is_one_json_object_like_the_others() {
        let mut buffer = [0u8; LINE];
        let time = Utc { year: 2026, month: 11, day: 2, hour: 9, minute: 8, second: 48, millis: 7 };
        let len = crash_line(&mut buffer, time, 0xC000_0005, b"comdlg32.dll", 0x1a2b, 4242);
        let text = std::str::from_utf8(&buffer[..len]).unwrap();
        assert!(text.ends_with("}\n"));
        let line: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(line["event"], "app.crashed");
        assert_eq!(line["level"], "ERROR");
        assert_eq!(line["code"], "0xC0000005");
        assert_eq!(line["module"], "comdlg32.dll");
        assert_eq!(line["offset"], "0x1A2B");
        assert_eq!(line["thread_id"], 4242);
        assert_eq!(line["ts"], "2026-11-02T09:08:48.007Z");
        // The same text serde_json writes for the other lines: keys in order.
        assert_eq!(text.trim_end(), line.to_string());
    }

    #[test]
    fn a_stack_overflow_keeps_its_code() {
        let mut buffer = [0u8; LINE];
        let time = Utc { year: 2026, month: 1, day: 1, hour: 0, minute: 0, second: 0, millis: 0 };
        let len = crash_line(&mut buffer, time, 0xC000_00FD, b"", 0, 1);
        let line: serde_json::Value = serde_json::from_slice(&buffer[..len]).unwrap();
        assert_eq!(line["code"], "0xC00000FD");
        assert_eq!(line["module"], "");
    }

    #[test]
    fn only_the_module_file_name_is_kept() {
        let mut out = [0u8; MODULE];
        let path = wide(r"C:\Users\marker-user\AppData\Local\SpecDeck\specdeck.exe");
        let len = module_name(&path, &mut out);
        assert_eq!(&out[..len], b"specdeck.exe");
        let path = wide(r"C:\Program Files (x86)\Microsoft\EdgeWebView\EmbeddedBrowserWebView.dll");
        let len = module_name(&path, &mut out);
        assert_eq!(&out[..len], b"EmbeddedBrowserWebView.dll");
        // Quotes and other characters that would need escaping are dropped.
        let len = module_name(&wide("we\"ird \u{e9}name.dll"), &mut out);
        assert_eq!(&out[..len], b"weirdname.dll");
    }

    #[test]
    fn a_long_module_name_is_cut_and_the_line_still_fits() {
        let mut out = [0u8; MODULE];
        let long = "a".repeat(300) + ".dll";
        let len = module_name(&wide(&long), &mut out);
        assert_eq!(len, MODULE);
        let mut buffer = [0u8; LINE];
        let time = Utc { year: 2026, month: 1, day: 1, hour: 0, minute: 0, second: 0, millis: 0 };
        let len = crash_line(&mut buffer, time, u32::MAX, &out, u64::MAX, u32::MAX);
        let line: serde_json::Value = serde_json::from_slice(&buffer[..len]).unwrap();
        assert_eq!(line["offset"], "0xFFFFFFFFFFFFFFFF");
    }
}
