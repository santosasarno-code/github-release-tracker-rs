//! Small Win32 helpers shared by the GUI and the network code.
#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::Memory::*;
use windows_sys::Win32::System::Time::SystemTimeToTzSpecificLocalTime;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// UTF-16, null-terminated.
pub fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Read a null-terminated UTF-16 string.
pub unsafe fn from_wide_ptr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

pub fn from_wide_buf(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

pub fn hex(c: u32) -> u32 {
    rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn loword(v: usize) -> u32 {
    (v & 0xFFFF) as u32
}
pub fn hiword(v: usize) -> u32 {
    ((v >> 16) & 0xFFFF) as u32
}

pub fn now_iso() -> String {
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond)
    }
}

pub fn today_ymd() -> String {
    now_iso().chars().take(10).collect()
}

/// "HH:MM" local time of a unix timestamp.
pub fn local_hhmm(epoch: i64) -> String {
    unsafe {
        let ticks = (epoch + 11_644_473_600) as u64 * 10_000_000;
        let ft = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
        let mut utc: SYSTEMTIME = std::mem::zeroed();
        let mut local: SYSTEMTIME = std::mem::zeroed();
        if windows_sys::Win32::System::Time::FileTimeToSystemTime(&ft, &mut utc) == 0 || SystemTimeToTzSpecificLocalTime(null(), &utc, &mut local) == 0 {
            return String::new();
        }
        format!("{:02}:{:02}", local.wHour, local.wMinute)
    }
}

pub fn shell_open(url: &str) {
    unsafe {
        ShellExecuteW(null_mut(), w("open").as_ptr(), w(url).as_ptr(), null(), null(), SW_SHOWNORMAL);
    }
}

pub fn message_box(owner: HWND, text: &str, flags: u32) -> i32 {
    unsafe { MessageBoxW(owner, w(text).as_ptr(), w(crate::APP_NAME).as_ptr(), flags) }
}

pub fn copy_to_clipboard(owner: HWND, text: &str) -> bool {
    unsafe {
        if OpenClipboard(owner) == 0 {
            return false;
        }
        EmptyClipboard();
        let data = w(text);
        let bytes = data.len() * 2;
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes);
        let mut ok = false;
        if !h.is_null() {
            let p = GlobalLock(h) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
                GlobalUnlock(h);
                ok = !SetClipboardData(13u32 /* CF_UNICODETEXT */, h as *mut c_void as HANDLE).is_null();
            }
            if !ok {
                GlobalFree(h);
            }
        }
        CloseClipboard();
        ok
    }
}

// Static-control styles and a message that windows-sys does not export.
pub const SS_LEFT: u32 = 0x0;
pub const SS_ICON: u32 = 0x3;
pub const SS_NOTIFY: u32 = 0x100;
pub const SS_CENTERIMAGE: u32 = 0x200;
pub const SS_ENDELLIPSIS: u32 = 0x4000;
pub const EM_SETLIMIT: u32 = 197; // EM_SETLIMITTEXT
