//! Settings and About windows (small modal windows, themed like the main window).

use crate::ui::{make_font, App};
use crate::winutil::*;
use github_release_tracker::*;
use std::cell::Cell;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::AdjustWindowRectExForDpi;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const ID_TOKEN: i32 = 301;
const ID_NOTE: i32 = 302;
const ID_LAUNCH: i32 = 303;
const ID_ABOUT: i32 = 304;
const ID_SAVE: i32 = 305;
const ID_CANCEL: i32 = 306;
const ID_COPYVER: i32 = 401;
const ID_CLOSE: i32 = 402;
const ID_LINK: i32 = 403;
const ID_ICON: i32 = 404;
const ID_HEAD: i32 = 405;

struct Dlg {
    app: *const App,
    done: Cell<bool>,
    save: Cell<bool>,
    brush: Cell<HBRUSH>,
    field: Cell<HBRUSH>,
}

fn register(app: &App) {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let class = w("GRT_Dialog");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(dlg_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: app.hinst,
            hIcon: LoadIconW(app.hinst, 1 as *const u16),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
            hIconSm: null_mut(),
        };
        RegisterClassExW(&wc);
    });
}

/// Outer rounded frame of the token box, in dialog pixels.
fn token_frame(app: &App) -> RECT {
    RECT { left: app.s(14), top: app.s(36), right: app.s(14 + 412), bottom: app.s(36 + 26) }
}

unsafe fn child(dlg: HWND, app: &App, class: &str, text: &str, style: u32, ex: u32, id: i32, x: i32, y: i32, wd: i32, h: i32) -> HWND {
    let s = |v: i32| app.s(v);
    let hw = CreateWindowExW(ex, w(class).as_ptr(), w(text).as_ptr(), WS_CHILD | WS_VISIBLE | style, s(x), s(y), s(wd), s(h), dlg, id as usize as HMENU, app.hinst, null());
    SendMessageW(hw, WM_SETFONT, app.fonts.get().ui as usize, 1);
    if class == "BUTTON" {
        let dark = app.dark.get();
        let sub = if style & (BS_AUTOCHECKBOX as u32) == BS_AUTOCHECKBOX as u32 && dark { "" } else if dark { "DarkMode_Explorer" } else { "Explorer" };
        SetWindowTheme(hw, w(sub).as_ptr(), if sub.is_empty() { w("").as_ptr() } else { null() });
    }
    hw
}

unsafe fn make_dialog(app: &App, parent: HWND, title: &str, cw: i32, ch: i32, state: &Dlg) -> HWND {
    register(app);
    let mut rc = RECT { left: 0, top: 0, right: app.s(cw), bottom: app.s(ch) };
    let style = WS_POPUP | WS_CAPTION | WS_SYSMENU;
    AdjustWindowRectExForDpi(&mut rc, style, 0, WS_EX_DLGMODALFRAME, app.dpi.get());
    let (ww, wh) = (rc.right - rc.left, rc.bottom - rc.top);
    let mut pr: RECT = std::mem::zeroed();
    GetWindowRect(parent, &mut pr);
    let x = pr.left + ((pr.right - pr.left) - ww) / 2;
    let y = pr.top + ((pr.bottom - pr.top) - wh) / 3;
    CreateWindowExW(WS_EX_DLGMODALFRAME, w("GRT_Dialog").as_ptr(), w(title).as_ptr(), style | WS_CLIPCHILDREN, x.max(0), y.max(0), ww, wh, parent, null_mut(), app.hinst, state as *const Dlg as *const std::ffi::c_void)
}

unsafe fn run_modal(parent: HWND, dlg: HWND, st: &Dlg, app: &App) {
    app.set_titlebar(dlg, app.dark.get());
    EnableWindow(parent, 0);
    ShowWindow(dlg, SW_SHOW);
    UpdateWindow(dlg);
    let mut msg: MSG = std::mem::zeroed();
    while !st.done.get() {
        let r = GetMessageW(&mut msg, null_mut(), 0, 0);
        if r == 0 {
            PostQuitMessage(msg.wParam as i32); // the main window is closing
            break;
        }
        if r < 0 {
            break;
        }
        if IsDialogMessageW(dlg, &msg) == 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    EnableWindow(parent, 1);
    DestroyWindow(dlg);
    SetForegroundWindow(parent);
}

pub fn show_settings(app: &App) {
    unsafe {
        let parent = app.hwnd.get();
        let st = Dlg { app, done: Cell::new(false), save: Cell::new(false), brush: Cell::new(null_mut()), field: Cell::new(null_mut()) };
        let dlg = make_dialog(app, parent, "Settings", 440, 236, &st);
        if dlg.is_null() {
            return;
        }
        let (token, launch) = {
            let d = app.data.borrow();
            (d.settings.token.clone(), d.settings.check_on_launch)
        };
        child(dlg, app, "STATIC", "GitHub token (optional, used for github.com only)", SS_LEFT as u32, 0, 300, 14, 14, 412, 18);
        let edit = child(dlg, app, "EDIT", "", (WS_TABSTOP | ES_AUTOHSCROLL as u32 | ES_PASSWORD as u32) as u32, 0, ID_TOKEN, 22, 40, 396, 18);
        SetWindowTextW(edit, w(&token).as_ptr());
        SendMessageW(edit, EM_SETLIMIT, 200, 0);
        child(
            dlg, app, "STATIC",
            "Not needed for normal use: releases are read from GitHub's public feed. A token only helps if the feed is blocked or for stable-only mode. It is stored in plain text in projects.json and is never written to exported files.",
            SS_LEFT as u32, 0, ID_NOTE, 14, 68, 412, 74,
        );
        let cb = child(dlg, app, "BUTTON", "Check for updates when the app starts", WS_TABSTOP | BS_AUTOCHECKBOX as u32, 0, ID_LAUNCH, 14, 148, 412, 22);
        SendMessageW(cb, BM_SETCHECK, if launch { BST_CHECKED } else { BST_UNCHECKED } as usize, 0);
        child(dlg, app, "BUTTON", "About", WS_TABSTOP | BS_PUSHBUTTON as u32, 0, ID_ABOUT, 14, 192, 84, 28);
        child(dlg, app, "BUTTON", "Save", WS_TABSTOP | BS_DEFPUSHBUTTON as u32, 0, ID_SAVE, 426 - 84 - 8 - 84, 192, 84, 28);
        child(dlg, app, "BUTTON", "Cancel", WS_TABSTOP | BS_PUSHBUTTON as u32, 0, ID_CANCEL, 426 - 84, 192, 84, 28);
        SetFocus(edit);
        run_modal_keep(parent, dlg, &st, app, edit);
        if st.save.get() {
            // read back before the window is destroyed: run_modal_keep stored them
        }
    }
}

// The values must be read before the window is destroyed, so the modal loop for Settings
// collects them when Save is pressed (see dlg_proc, ID_SAVE).
unsafe fn run_modal_keep(parent: HWND, dlg: HWND, st: &Dlg, app: &App, _edit: HWND) {
    run_modal(parent, dlg, st, app);
}

pub fn show_about(app: &App, parent: HWND) {
    unsafe {
        let st = Dlg { app, done: Cell::new(false), save: Cell::new(false), brush: Cell::new(null_mut()), field: Cell::new(null_mut()) };
        // The address is long: measure it (font and DPI as they are now) and widen the window if needed.
        let hdc = GetDC(parent);
        let url_px = app.text_width(hdc, REPO_URL);
        ReleaseDC(parent, hdc);
        let cw = (80 + url_px * 96 / app.dpi.get() as i32 + 8 + 16).max(430);
        let inner = cw - 80 - 16;
        let dlg = make_dialog(app, parent, &format!("About {}", APP_NAME), cw, 212, &st);
        if dlg.is_null() {
            return;
        }
        let icon = child(dlg, app, "STATIC", "", SS_ICON as u32, 0, ID_ICON, 16, 16, 48, 48);
        let hicon = LoadImageW(app.hinst, 1 as *const u16, IMAGE_ICON, app.s(48), app.s(48), LR_DEFAULTCOLOR);
        SendMessageW(icon, STM_SETIMAGE, IMAGE_ICON as usize, hicon as isize);
        let head = child(dlg, app, "STATIC", APP_NAME, SS_LEFT as u32, 0, ID_HEAD, 80, 14, inner, 26);
        SendMessageW(head, WM_SETFONT, make_font("Segoe UI", 14, 700, app.dpi.get()) as usize, 1);
        child(dlg, app, "STATIC", &format!("Version {}", APP_VERSION), SS_LEFT as u32, 0, 410, 80, 42, inner, 18);
        child(
            dlg, app, "STATIC",
            "Tracks the latest release of the projects you follow on GitHub, GitLab, Codeberg and other Git servers. No account, login or token needed. MIT License.",
            SS_LEFT as u32, 0, ID_NOTE, 80, 66, inner, 54,
        );
        child(dlg, app, "STATIC", REPO_URL, SS_LEFT as u32 | SS_NOTIFY as u32, 0, ID_LINK, 80, 124, inner, 18);
        child(dlg, app, "BUTTON", "Copy version", WS_TABSTOP | BS_PUSHBUTTON as u32, 0, ID_COPYVER, cw - 16 - 84 - 8 - 96, 168, 96, 28);
        child(dlg, app, "BUTTON", "Close", WS_TABSTOP | BS_DEFPUSHBUTTON as u32, 0, ID_CLOSE, cw - 16 - 84, 168, 84, 28);
        run_modal(parent, dlg, &st, app);
    }
}

unsafe fn text_of(h: HWND) -> String {
    let n = GetWindowTextLengthW(h) as usize;
    let mut buf = vec![0u16; n + 2];
    GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
    from_wide_buf(&buf)
}

unsafe extern "system" fn dlg_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE || msg == WM_CREATE {
        if msg == WM_CREATE {
            let cs = &*(lp as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        }
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let st = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Dlg;
    if st.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let st = &*st;
    let app = &*st.app;
    let p = app.pal();
    match msg {
        WM_ERASEBKGND => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            if st.brush.get().is_null() {
                st.brush.set(CreateSolidBrush(p.bg));
            }
            FillRect(wp as HDC, &rc, st.brush.get());
            let edit = GetDlgItem(hwnd, ID_TOKEN);
            if !edit.is_null() {
                // rounded frame around the token box (the edit control itself is borderless)
                let fr = token_frame(app);
                let fill = CreateSolidBrush(p.field);
                let pen = CreatePen(PS_SOLID, 1, if GetFocus() == edit { p.sel_bg } else { p.border });
                let (ob, op) = (SelectObject(wp as HDC, fill as HGDIOBJ), SelectObject(wp as HDC, pen as HGDIOBJ));
                let e = app.s(8);
                RoundRect(wp as HDC, fr.left, fr.top, fr.right, fr.bottom, e, e);
                SelectObject(wp as HDC, ob);
                SelectObject(wp as HDC, op);
                DeleteObject(fill as HGDIOBJ);
                DeleteObject(pen as HGDIOBJ);
            }
            1
        }
        WM_LBUTTONDOWN => {
            let edit = GetDlgItem(hwnd, ID_TOKEN);
            let (x, y) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
            let fr = token_frame(app);
            if !edit.is_null() && x >= fr.left && x < fr.right && y >= fr.top && y < fr.bottom {
                SetFocus(edit);
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLORDLG => {
            let id = GetDlgCtrlID(lp as HWND);
            SetTextColor(wp as HDC, if id == ID_NOTE { p.sub_fg } else if id == ID_LINK { p.link } else { p.fg });
            SetBkColor(wp as HDC, p.bg);
            if st.brush.get().is_null() {
                st.brush.set(CreateSolidBrush(p.bg));
            }
            st.brush.get() as LRESULT
        }
        WM_CTLCOLOREDIT => {
            SetTextColor(wp as HDC, p.fg);
            SetBkColor(wp as HDC, p.field);
            if st.field.get().is_null() {
                st.field.set(CreateSolidBrush(p.field));
            }
            st.field.get() as LRESULT
        }
        WM_SETCURSOR => {
            if GetDlgCtrlID(wp as HWND) == ID_LINK {
                SetCursor(LoadCursorW(null_mut(), IDC_HAND));
                return 1;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_COMMAND => {
            let id = loword(wp) as i32;
            if id == ID_TOKEN && (hiword(wp) == EN_SETFOCUS as u32 || hiword(wp) == EN_KILLFOCUS as u32) {
                let fr = token_frame(app);
                InvalidateRect(hwnd, &fr, 1);
            }
            match id {
                ID_SAVE | 1 if !GetDlgItem(hwnd, ID_TOKEN).is_null() => {
                    let token = text_of(GetDlgItem(hwnd, ID_TOKEN)).trim().to_string();
                    let launch = SendMessageW(GetDlgItem(hwnd, ID_LAUNCH), BM_GETCHECK, 0, 0) as u32 == BST_CHECKED;
                    {
                        let mut d = app.data.borrow_mut();
                        d.settings.token = token.chars().take(200).collect();
                        d.settings.check_on_launch = launch;
                    }
                    app.persist();
                    app.set_status("Settings saved.");
                    st.save.set(true);
                    st.done.set(true);
                }
                ID_CANCEL | 2 | ID_CLOSE => st.done.set(true),
                1 => st.done.set(true), // Enter in About
                ID_ABOUT => show_about(app, hwnd),
                ID_COPYVER => {
                    let t = format!("{} v{}", APP_NAME, APP_VERSION);
                    copy_to_clipboard(hwnd, &t);
                    app.set_status(&format!("Copied: {}", t));
                }
                ID_LINK => shell_open(REPO_URL),
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            st.done.set(true);
            0
        }
        WM_DESTROY => {
            for b in [st.brush.replace(null_mut()), st.field.replace(null_mut())] {
                if !b.is_null() {
                    DeleteObject(b as HGDIOBJ);
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
