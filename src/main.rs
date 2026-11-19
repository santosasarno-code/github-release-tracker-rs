//! GitHub Release Tracker (Rust / Win32). Native window, native list view, no GUI framework.

#![windows_subsystem = "windows"]
#![allow(non_snake_case, non_upper_case_globals, clippy::missing_safety_doc, clippy::too_many_lines)]

mod dialogs;
mod http;
mod ui;
mod winutil;

pub use github_release_tracker::*;
use std::sync::atomic::{AtomicPtr, Ordering};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use winutil::*;

pub const APP_ID: &str = "santosasarno-code.GitHubReleaseTracker";

pub static APP: AtomicPtr<ui::App> = AtomicPtr::new(std::ptr::null_mut());

fn main() {
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(w(APP_ID).as_ptr());
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_BAR_CLASSES | ICC_STANDARD_CLASSES | ICC_TAB_CLASSES,
        };
        InitCommonControlsEx(&icc);
        let hinst = GetModuleHandleW(std::ptr::null());
        let app = Box::into_raw(Box::new(ui::App::new(hinst)));
        APP.store(app, Ordering::SeqCst);
        if !(*app).create_main_window() {
            message_box(std::ptr::null_mut(), "Could not create the window.", MB_OK | MB_ICONERROR);
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if (*app).pre_translate(&msg) {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
