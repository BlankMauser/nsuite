use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use skyline::hooks::A64HookFunction;
use skyline::libc::{c_char, c_void, strlen};

use crate::symbol;

type SkylineTcpSendRawFn = unsafe extern "C" fn(bytes: *const u8, len: u64);

#[derive(Default)]
struct LoggerState {
    file: Option<File>,
    file_path: Option<String>,
}

static STATE: OnceLock<Mutex<LoggerState>> = OnceLock::new();
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static MIRROR_ALL_PLUGINS: AtomicBool = AtomicBool::new(false);
static mut SKYLINE_TCP_SEND_RAW_ORIG: Option<SkylineTcpSendRawFn> = None;

const DEFAULT_LOG_PATH: &str = "sd:/ssbusync.log";
const SKYLINE_TCP_SEND_RAW_SYMBOL: &[u8] = b"skyline_tcp_send_raw\0";

unsafe extern "C" {
    fn skyline_tcp_send_raw(bytes: *const u8, len: u64);
}

#[inline(always)]
fn state() -> &'static Mutex<LoggerState> {
    STATE.get_or_init(|| Mutex::new(LoggerState::default()))
}

#[inline(always)]
fn open_log_file(path: &str) -> Option<File> {
    if let Err(err) = std::fs::remove_file(path) {
        if err.kind() != std::io::ErrorKind::NotFound {
            // Fall back to truncate-on-open if delete fails.
        }
    }
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .ok()
}

#[inline(always)]
fn write_file_bytes(bytes: &[u8]) {
    if let Ok(mut guard) = state().lock() {
        if let Some(file) = guard.file.as_mut() {
            let _ = file.write_all(bytes);
        }
    }
}

#[inline(always)]
fn write_file_line(text: &str) {
    if let Ok(mut guard) = state().lock() {
        if let Some(file) = guard.file.as_mut() {
            let _ = file.write_all(text.as_bytes());
            let _ = file.write_all(b"\n");
        }
    }
}

#[inline(always)]
fn emit_tcp_raw(bytes: &[u8]) {
    unsafe {
        if let Some(orig) = SKYLINE_TCP_SEND_RAW_ORIG {
            orig(bytes.as_ptr(), bytes.len() as u64);
        } else {
            skyline_tcp_send_raw(bytes.as_ptr(), bytes.len() as u64);
        }
    }
}

#[inline(always)]
fn should_mirror_message(message: &str) -> bool {
    if MIRROR_ALL_PLUGINS.load(Ordering::Acquire) {
        return true;
    }
    message.contains("[nsuite]")
}

/// Initializes file logging. Returns `true` when the log file is open.
pub fn init(path: Option<&str>) -> bool {
    let path = path.unwrap_or(DEFAULT_LOG_PATH);
    let file = open_log_file(path);
    if let Ok(mut guard) = state().lock() {
        guard.file = file;
        guard.file_path = Some(path.to_string());
        guard.file.is_some()
    } else {
        false
    }
}

#[inline(always)]
pub fn init_default() -> bool {
    init(None)
}

#[inline(always)]
pub fn current_file_path() -> Option<String> {
    if let Ok(guard) = state().lock() {
        guard.file_path.clone()
    } else {
        None
    }
}

#[inline(always)]
pub fn log(text: &str) {
    write_file_bytes(text.as_bytes());
    emit_tcp_raw(text.as_bytes());
}

#[inline(always)]
pub fn log_line(text: &str) {
    write_file_line(text);
    let mut buf = String::with_capacity(text.len() + 1);
    buf.push_str(text);
    buf.push('\n');
    emit_tcp_raw(buf.as_bytes());
}

#[inline(always)]
pub fn log_fmt(args: core::fmt::Arguments<'_>) {
    log(&std::fmt::format(args));
}

#[inline(always)]
pub fn log_line_fmt(args: core::fmt::Arguments<'_>) {
    log_line(&std::fmt::format(args));
}

unsafe extern "C" fn skyline_tcp_send_raw_hook(bytes: *const u8, len: u64) {
    if !bytes.is_null() && len > 0 {
        let len = len as usize;
        let data = core::slice::from_raw_parts(bytes, len);
        if let Ok(text) = core::str::from_utf8(data) {
            if should_mirror_message(text) {
                write_file_bytes(data);
            }
        } else if MIRROR_ALL_PLUGINS.load(Ordering::Acquire) {
            write_file_bytes(data);
        }
    }

    if let Some(orig) = SKYLINE_TCP_SEND_RAW_ORIG {
        orig(bytes, len);
    }
}

/// Hooks skyline's global `skyline_tcp_send_raw` so logs from all plugins are mirrored to the file sink.
pub unsafe fn install_global_skyline_log_hook() -> bool {
    if HOOK_INSTALLED.load(Ordering::Acquire) {
        return true;
    }

    let target_addr = match symbol::lookup_symbol_addr(SKYLINE_TCP_SEND_RAW_SYMBOL) {
        Some(addr) => addr,
        None => return false,
    };

    let mut trampoline: *mut c_void = core::ptr::null_mut();
    A64HookFunction(
        target_addr as *const c_void,
        skyline_tcp_send_raw_hook as *const c_void,
        &mut trampoline,
    );

    if trampoline.is_null() {
        return false;
    }

    SKYLINE_TCP_SEND_RAW_ORIG = Some(core::mem::transmute(trampoline));
    HOOK_INSTALLED.store(true, Ordering::Release);
    true
}

#[inline(always)]
pub fn global_skyline_log_hook_installed() -> bool {
    HOOK_INSTALLED.load(Ordering::Acquire)
}

#[inline(always)]
pub fn set_mirror_all_plugins(enabled: bool) {
    MIRROR_ALL_PLUGINS.store(enabled, Ordering::Release);
}

#[inline(always)]
pub fn mirror_all_plugins() -> bool {
    MIRROR_ALL_PLUGINS.load(Ordering::Acquire)
}

#[inline(always)]
pub unsafe fn write_c_string_line(c_str: *const c_char) {
    if c_str.is_null() {
        return;
    }
    let len = strlen(c_str) as usize;
    let data = core::slice::from_raw_parts(c_str as *const u8, len);
    if let Ok(text) = core::str::from_utf8(data) {
        log_line(text);
    } else {
        write_file_bytes(data);
        write_file_bytes(b"\n");
    }
}
