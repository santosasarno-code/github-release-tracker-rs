//! The main window: toolbar, list view, status bar, dark/light theme, DPI scaling.

use crate::dialogs;
use crate::http::WinHttpNet;
use crate::winutil::*;
use crate::APP;
use github_release_tracker::*;
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Storage::FileSystem::GetFileAttributesW;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// control ids
pub const ID_ENTRY: i32 = 101;
pub const ID_ADD: i32 = 102;
pub const ID_CHECK: i32 = 103;
pub const ID_SEEN: i32 = 104;
pub const ID_OPEN: i32 = 105;
pub const ID_REMOVE: i32 = 106;
pub const ID_SETTINGS: i32 = 107;
pub const ID_THEME: i32 = 108;
pub const ID_EXPORT: i32 = 109;
pub const ID_IMPORT: i32 = 110;
pub const ID_LIST: i32 = 111;
pub const ID_STATUS: i32 = 112;
pub const ID_LABEL: i32 = 113;
pub const ID_PENCIL: i32 = 114;
pub const ID_FLABEL: i32 = 115;
pub const ID_FILTER: i32 = 116;
pub const ID_FKIND: i32 = 117;
pub const ID_FCLEAR: i32 = 118;
pub const ID_FCOUNT: i32 = 119;
const TIMER_PENCIL: usize = 77;
const TIMER_FILTER: usize = 78;
const M_KIND: u32 = 300; // 301..=305 pick a status filter
const G_EDIT: u16 = 0xE70F;
// popup menu ids
const M_OPEN: u32 = 201;
const M_REPO: u32 = 202;
const M_SEEN: u32 = 203;
const M_RENAME: u32 = 204;
const M_RESET: u32 = 205;
const M_PRE: u32 = 206;
const M_REMOVE: u32 = 207;
const M_COPY: u32 = 208;
// private window messages
const WM_JOB: u32 = WM_APP + 1;
const WM_RELAYOUT: u32 = WM_APP + 2;
const WM_RENAMED: u32 = WM_APP + 3;
const WM_TRACKED: u32 = WM_APP + 4;
const WM_STARTUP: u32 = WM_APP + 5;

const COLS: [(&str, &str); 7] = [
    ("name", "Project"),
    ("repo", "Repository"),
    ("source", "Source"),
    ("release", "Current release"),
    ("published", "Published"),
    ("status", "Status"),
    ("checked", "Last checked"),
];

// Segoe MDL2 Assets glyphs (present on Windows 10 and 11)
const G_REFRESH: u16 = 0xE72C;
const G_EYE: u16 = 0xE73A; // checkbox with a check mark (was an eye)
const G_OPEN: u16 = 0xE8A7;
const G_TRASH: u16 = 0xE74D;
const G_GEAR: u16 = 0xE713;
const G_PLUS: u16 = 0xE710;
const G_IMPORT: u16 = 0xE896;
const G_EXPORT: u16 = 0xE898;
const G_MOON: u16 = 0xE708;
const G_SUN: u16 = 0xE706;
const G_FILTER: u16 = 0xE71C;
const G_CLEAR: u16 = 0xE894;

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: u32,
    pub fg: u32,
    pub field: u32,
    pub sel_bg: u32,
    pub sel_fg: u32,
    pub btn_bg: u32,
    pub btn_active: u32,
    pub border: u32,
    pub update_bg: u32,
    pub update_fg: u32,
    pub error_fg: u32,
    pub none_fg: u32,
    pub sub_fg: u32,
    pub line: u32,
    pub link: u32,
    pub head_bg: u32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: hex(0x202124), fg: hex(0xe8eaed), field: hex(0x2b2d31), sel_bg: hex(0x2f6fbf), sel_fg: hex(0xffffff),
            btn_bg: hex(0x3a3d42), btn_active: hex(0x4a4d52), border: hex(0x4a4d52), update_bg: hex(0x4d4210),
            update_fg: hex(0xffe9a6), error_fg: hex(0xff8a80), none_fg: hex(0x9aa0a6), sub_fg: hex(0x9aa0a6),
            line: hex(0x3d4046), link: hex(0x4da3ff), head_bg: hex(0x33363b),
        }
    } else {
        Palette {
            bg: hex(0xf3f3f3), fg: hex(0x1b1b1b), field: hex(0xffffff), sel_bg: hex(0x0078d4), sel_fg: hex(0xffffff),
            btn_bg: hex(0xe1e1e1), btn_active: hex(0xd0d0d0), border: hex(0xbdbdbd), update_bg: hex(0xfff3cd),
            update_fg: hex(0x1b1b1b), error_fg: hex(0xb00020), none_fg: hex(0x777777), sub_fg: hex(0x666666),
            line: hex(0xd4d4d4), link: hex(0x0b57d0), head_bg: hex(0xe5e5e5),
        }
    }
}

#[derive(Clone, Copy)]
struct Ctl {
    label: HWND,
    entry: HWND,
    flabel: HWND,
    filter: HWND,
    fcount: HWND,
    list: HWND,
    header: HWND,
    status: HWND,
    tip: HWND,
}

#[derive(Clone, Copy)]
pub struct Fonts {
    pub ui: HFONT,
    pub bold: HFONT,
    pub icon: HFONT,
}

#[derive(Clone, Copy)]
struct Brushes {
    bg: HBRUSH,
    field: HBRUSH,
}

pub enum Job {
    Added(Result<(RepoInfo, Option<Release>), String>),
    Checked { results: Vec<(String, Option<Release>, Option<String>)>, rate: Option<i64> },
}

pub struct App {
    pub hinst: HINSTANCE,
    pub hwnd: Cell<HWND>,
    ctl: Cell<Ctl>,
    pub data: RefCell<Data>,
    view: RefCell<Vec<usize>>,
    pub dark: Cell<bool>,
    pub dpi: Cell<u32>,
    pub fonts: Cell<Fonts>,
    brushes: Cell<Brushes>,
    busy: Cell<bool>,
    suppress: Cell<bool>,
    col_base: RefCell<Vec<i32>>, // preferred widths at 96 DPI
    net: Arc<WinHttpNet>,
    data_path: PathBuf,
    sort_col: RefCell<String>,
    sort_asc: Cell<bool>,
    tip_text: RefCell<Vec<u16>>,
    pending_rename: RefCell<Option<(String, String)>>,
    status_text: RefCell<String>,
    pencil: Cell<HWND>,
    pencil_row: Cell<isize>,
    entry_frame: Cell<RECT>,
    filter_frame: Cell<RECT>,
    filter_kind: RefCell<String>,
}

fn null_ctl() -> Ctl {
    Ctl { label: null_mut(), entry: null_mut(), flabel: null_mut(), filter: null_mut(), fcount: null_mut(), list: null_mut(), header: null_mut(), status: null_mut(), tip: null_mut() }
}

fn data_file() -> PathBuf {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(|| PathBuf::from("."));
    // Portable first: next to the exe. If that folder is read-only, use %APPDATA%.
    let probe = exe_dir.join(format!(".write_test_{}", std::process::id()));
    if std::fs::write(&probe, b"x").is_ok() {
        let _ = std::fs::remove_file(&probe);
        return exe_dir.join("projects.json");
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let d = PathBuf::from(appdata).join("GitHubReleaseTracker");
        if std::fs::create_dir_all(&d).is_ok() {
            return d.join("projects.json");
        }
    }
    exe_dir.join("projects.json")
}

fn load_data(path: &PathBuf) -> Data {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_data(&text).unwrap_or_else(|| {
            let mut bak = path.clone().into_os_string();
            bak.push(".bak");
            let _ = std::fs::rename(path, PathBuf::from(bak));
            Data::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data::default(),
        Err(_) => {
            let mut bak = path.clone().into_os_string();
            bak.push(".bak");
            let _ = std::fs::rename(path, PathBuf::from(bak));
            Data::default()
        }
    }
}

fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text.as_bytes())?;
    std::fs::rename(&tmp, path)
}

fn system_dark() -> bool {
    use windows_sys::Win32::System::Registry::*;
    unsafe {
        let mut v: u32 = 1;
        let mut sz = 4u32;
        let r = RegGetValueW(
            HKEY_CURRENT_USER,
            w("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            w("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            &mut v as *mut u32 as *mut c_void,
            &mut sz,
        );
        r == 0 && v == 0
    }
}

pub fn make_font(face: &str, pt: i32, weight: i32, dpi: u32) -> HFONT {
    unsafe { CreateFontW(-(pt * dpi as i32 / 72), 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, w(face).as_ptr()) }
}

impl App {
    pub fn new(hinst: HINSTANCE) -> App {
        let data_path = data_file();
        let data = load_data(&data_path);
        let dark = match data.settings.theme.as_str() {
            "dark" => true,
            "light" => false,
            _ => system_dark(),
        };
        let sort_col = data.settings.sort_col.clone();
        let sort_asc = data.settings.sort_asc;
        App {
            hinst,
            hwnd: Cell::new(null_mut()),
            ctl: Cell::new(null_ctl()),
            data: RefCell::new(data),
            view: RefCell::new(Vec::new()),
            dark: Cell::new(dark),
            dpi: Cell::new(96),
            fonts: Cell::new(Fonts { ui: null_mut(), bold: null_mut(), icon: null_mut() }),
            brushes: Cell::new(Brushes { bg: null_mut(), field: null_mut() }),
            busy: Cell::new(false),
            suppress: Cell::new(false),
            col_base: RefCell::new(COL_BASE.to_vec()),
            net: Arc::new(WinHttpNet::new()),
            data_path,
            sort_col: RefCell::new(sort_col),
            sort_asc: Cell::new(sort_asc),
            tip_text: RefCell::new(w("")),
            pending_rename: RefCell::new(None),
            status_text: RefCell::new(String::new()),
            pencil: Cell::new(null_mut()),
            pencil_row: Cell::new(-1),
            entry_frame: Cell::new(RECT { left: 0, top: 0, right: 0, bottom: 0 }),
            filter_frame: Cell::new(RECT { left: 0, top: 0, right: 0, bottom: 0 }),
            filter_kind: RefCell::new("all".to_string()),
        }
    }

    pub fn pal(&self) -> Palette {
        palette(self.dark.get())
    }

    pub fn s(&self, v: i32) -> i32 {
        v * self.dpi.get() as i32 / 96
    }

    // ------------------------------------------------------------------ window
    pub fn create_main_window(&self) -> bool {
        unsafe {
            let class = w("GRT_MainWindow");
            let icon = LoadIconW(self.hinst, 1 as *const u16);
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: 0, // no CS_HREDRAW/CS_VREDRAW: controls are laid out by hand, a full repaint per resize step is wasted work
                lpfnWndProc: Some(main_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: self.hinst,
                hIcon: icon,
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class.as_ptr(),
                hIconSm: icon,
            };
            RegisterClassExW(&wc);
            let title = format!("{} v{}", APP_NAME, APP_VERSION);
            let hwnd = CreateWindowExW(
                0, class.as_ptr(), w(&title).as_ptr(), WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT, CW_USEDEFAULT, 1260, 600, null_mut(), null_mut(), self.hinst, null(),
            );
            if hwnd.is_null() {
                return false;
            }
            // Start size: wide enough for every column, never bigger than the screen; centered.
            let dpi = self.dpi.get() as i32;
            let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
            let mut rc = RECT { left: 0, top: 0, right: 1260 * dpi / 96, bottom: 560 * dpi / 96 };
            AdjustWindowRectExForDpi(&mut rc, WS_OVERLAPPEDWINDOW, 0, 0, dpi as u32);
            let (mut ww, mut wh) = (rc.right - rc.left, rc.bottom - rc.top);
            ww = ww.min(sw - 40);
            wh = wh.min(sh - 80);
            SetWindowPos(hwnd, null_mut(), ((sw - ww) / 2).max(0), ((sh - wh) / 3).max(0), ww, wh, SWP_NOZORDER);
            ShowWindow(hwnd, SW_SHOWNORMAL);
            UpdateWindow(hwnd);
            PostMessageW(hwnd, WM_STARTUP, 0, 0);
            true
        }
    }

    pub fn pre_translate(&self, msg: &MSG) -> bool {
        unsafe {
            let main = self.hwnd.get();
            if main.is_null() {
                return false;
            }
            let ctl = self.ctl.get();
            let focus = GetFocus();
            // The rename box lives inside the list view: it must get Enter/Esc itself.
            if !focus.is_null() && GetParent(focus) == ctl.list {
                return false;
            }
            if msg.hwnd == main || IsChild(main, msg.hwnd) != 0 {
                if msg.message == WM_KEYDOWN && msg.wParam == 'F' as usize && GetKeyState(VK_CONTROL as i32) < 0 {
                    SetFocus(ctl.filter);
                    SendMessageW(ctl.filter, EM_SETSEL, 0, -1);
                    return true;
                }
                return IsDialogMessageW(main, msg) != 0;
            }
            false
        }
    }

    fn on_create(&self, hwnd: HWND) {
        unsafe {
            self.hwnd.set(hwnd);
            let d = GetDpiForWindow(hwnd);
            self.dpi.set(if d == 0 { 96 } else { d });
            self.make_fonts();
            let hi = self.hinst;
            let mk = |class: &str, text: &str, style: u32, ex: u32, id: i32| -> HWND {
                CreateWindowExW(ex, w(class).as_ptr(), w(text).as_ptr(), WS_CHILD | WS_VISIBLE | style, 0, 0, 10, 10, hwnd, id as usize as HMENU, hi, null())
            };
            let label = mk("STATIC", "Project (repo URL or owner/repo):", SS_LEFT as u32 | SS_CENTERIMAGE as u32, 0, ID_LABEL);
            let entry = mk("EDIT", "", WS_TABSTOP | ES_AUTOHSCROLL as u32, 0, ID_ENTRY);
            let flabel = mk("STATIC", "Filter:", SS_LEFT as u32 | SS_CENTERIMAGE as u32, 0, ID_FLABEL);
            let filter = mk("EDIT", "", WS_TABSTOP | ES_AUTOHSCROLL as u32, 0, ID_FILTER);
            let fcount = mk("STATIC", "", 0x2u32 /* SS_RIGHT */ | SS_CENTERIMAGE as u32, 0, ID_FCOUNT);
            for (id, glyph, text) in BUTTONS {
                let _ = glyph;
                mk("BUTTON", text, WS_TABSTOP | BS_OWNERDRAW as u32, 0, id);
            }
            let list = mk(
                "SysListView32", "",
                WS_TABSTOP | WS_CLIPSIBLINGS | LVS_REPORT as u32 | LVS_OWNERDATA as u32 | LVS_SHOWSELALWAYS as u32 | LVS_EDITLABELS as u32,
                0, ID_LIST,
            );
            let status = mk("STATIC", "Add a project to get started.", SS_LEFT as u32 | SS_CENTERIMAGE as u32 | SS_NOTIFY as u32 | SS_ENDELLIPSIS as u32, 0, ID_STATUS);
            let tip = CreateWindowExW(
                WS_EX_TOPMOST, w("tooltips_class32").as_ptr(), null(), WS_POPUP | TTS_NOPREFIX | TTS_ALWAYSTIP,
                CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, hwnd, null_mut(), hi, null(),
            );
            SendMessageW(list, LVM_SETEXTENDEDLISTVIEWSTYLE, 0, (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | LVS_EX_LABELTIP) as isize);
            let header = SendMessageW(list, LVM_GETHEADER, 0, 0) as HWND;
            self.ctl.set(Ctl { label, entry, flabel, filter, fcount, list, header, status, tip });
            for (i, (_, title)) in COLS.iter().enumerate() {
                let mut text = w(title);
                let mut col: LVCOLUMNW = std::mem::zeroed();
                col.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_FMT;
                col.fmt = LVCFMT_LEFT;
                col.cx = self.s(COL_BASE[i]);
                col.pszText = text.as_mut_ptr();
                SendMessageW(list, LVM_INSERTCOLUMNW, i, &col as *const LVCOLUMNW as isize);
            }
            SendMessageW(entry, EM_SETLIMIT, 300, 0);
            SendMessageW(filter, EM_SETLIMIT, 100, 0);
            // hover pencil (rename): a small owner-drawn button that floats over the list
            let pencil = mk("BUTTON", "", BS_OWNERDRAW as u32 | WS_CLIPSIBLINGS, 0, ID_PENCIL);
            ShowWindow(pencil, SW_HIDE);
            self.pencil.set(pencil);
            windows_sys::Win32::UI::Shell::SetWindowSubclass(list, Some(list_sub), 1, 0);
            windows_sys::Win32::UI::Shell::SetWindowSubclass(header, Some(header_sub), 2, 0);
            // tooltip for the icon-only theme button
            let mut ti: TTTOOLINFOW = std::mem::zeroed();
            ti.cbSize = std::mem::size_of::<TTTOOLINFOW>() as u32;
            ti.uFlags = TTF_IDISHWND | TTF_SUBCLASS;
            ti.hwnd = hwnd;
            ti.uId = GetDlgItem(hwnd, ID_THEME) as usize;
            ti.lpszText = -1isize as *mut u16; // LPSTR_TEXTCALLBACK
            SendMessageW(tip, TTM_ADDTOOLW, 0, &ti as *const TTTOOLINFOW as isize);
            self.apply_fonts();
            self.apply_theme();
            self.update_headings();
            self.render();
            self.layout();
        }
    }

    fn make_fonts(&self) {
        let dpi = self.dpi.get();
        let old = self.fonts.get();
        self.fonts.set(Fonts { ui: make_font("Segoe UI", 9, 400, dpi), bold: make_font("Segoe UI", 9, 700, dpi), icon: make_font("Segoe MDL2 Assets", 10, 400, dpi) });
        unsafe {
            for f in [old.ui, old.bold, old.icon] {
                if !f.is_null() {
                    DeleteObject(f as HGDIOBJ);
                }
            }
        }
    }

    fn apply_fonts(&self) {
        unsafe {
            let (f, c) = (self.fonts.get(), self.ctl.get());
            for h in [c.label, c.entry, c.flabel, c.filter, c.fcount, c.list, c.status] {
                SendMessageW(h, WM_SETFONT, f.ui as usize, 1);
            }
            // The list view hands its font to the header; set the bold one afterwards.
            SendMessageW(c.header, WM_SETFONT, f.bold as usize, 1);
        }
        self.refresh_list_layout();
    }

    // ------------------------------------------------------------------ theme
    pub fn apply_theme(&self) {
        unsafe {
            let (p, c) = (self.pal(), self.ctl.get());
            let old = self.brushes.get();
            self.brushes.set(Brushes { bg: CreateSolidBrush(p.bg), field: CreateSolidBrush(p.field) });
            for b in [old.bg, old.field] {
                if !b.is_null() {
                    DeleteObject(b as HGDIOBJ);
                }
            }
            let dark = self.dark.get();
            SendMessageW(c.list, LVM_SETBKCOLOR, 0, p.field as isize);
            SendMessageW(c.list, LVM_SETTEXTCOLOR, 0, p.fg as isize);
            SendMessageW(c.list, LVM_SETTEXTBKCOLOR, 0, p.field as isize);
            SetWindowTheme(c.list, w(if dark { "DarkMode_Explorer" } else { "Explorer" }).as_ptr(), null());
            SetWindowTheme(c.header, w("").as_ptr(), w("").as_ptr()); // the header is drawn by header_draw()
            self.set_titlebar(self.hwnd.get(), dark);
            self.refresh_list_layout();
            RedrawWindow(self.hwnd.get(), null(), null_mut(), RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_FRAME);
        }
    }

    /// The list view only re-measures its header on a size change; nudge it after the header's
    /// font or theme changed, otherwise the first row can end up under the header.
    fn refresh_list_layout(&self) {
        unsafe {
            let c = self.ctl.get();
            if c.list.is_null() {
                return;
            }
            let mut rc: RECT = std::mem::zeroed();
            GetWindowRect(c.list, &mut rc);
            let mut pt = POINT { x: rc.left, y: rc.top };
            windows_sys::Win32::Graphics::Gdi::ScreenToClient(self.hwnd.get(), &mut pt);
            let (wd, ht) = (rc.right - rc.left, rc.bottom - rc.top);
            MoveWindow(c.list, pt.x, pt.y, wd + 1, ht, 1);
            MoveWindow(c.list, pt.x, pt.y, wd, ht, 1);
        }
    }

    pub fn set_titlebar(&self, hwnd: HWND, dark: bool) {
        unsafe {
            let v: i32 = dark as i32;
            for attr in [20u32, 19] {
                if DwmSetWindowAttribute(hwnd, attr as _, &v as *const i32 as *const c_void, 4) == 0 {
                    break;
                }
            }
        }
    }

    fn toggle_theme(&self) {
        self.dark.set(!self.dark.get());
        self.data.borrow_mut().settings.theme = if self.dark.get() { "dark" } else { "light" }.into();
        self.persist();
        self.apply_theme();
        self.update_headings();
        self.set_status(if self.dark.get() { "Dark mode" } else { "Light mode" });
    }

    // ------------------------------------------------------------------ layout
    fn button_width(&self, hdc: HDC, text: &str) -> i32 {
        if text.is_empty() {
            return self.s(36);
        }
        self.s(12 + 16 + 6 + 12) + self.text_width(hdc, text)
    }

    pub fn text_width(&self, hdc: HDC, text: &str) -> i32 {
        unsafe {
            let old = SelectObject(hdc, self.fonts.get().ui as HGDIOBJ);
            let t = w(text);
            let mut sz = SIZE { cx: 0, cy: 0 };
            GetTextExtentPoint32W(hdc, t.as_ptr(), (t.len() - 1) as i32, &mut sz);
            SelectObject(hdc, old);
            sz.cx
        }
    }

    /// (left toolbar buttons, right toolbar buttons) as (id, width), plus the total width needed.
    fn toolbar_plan(&self) -> (Vec<(i32, i32)>, Vec<(i32, i32)>, i32) {
        unsafe {
            let hdc = GetDC(self.hwnd.get());
            let width = |id: i32| -> i32 {
                let text = BUTTONS.iter().find(|b| b.0 == id).map_or("", |b| b.2);
                self.button_width(hdc, text)
            };
            let left: Vec<(i32, i32)> = [ID_CHECK, ID_SEEN, ID_OPEN, ID_REMOVE].iter().map(|&i| (i, width(i))).collect();
            let right: Vec<(i32, i32)> = [ID_SETTINGS, ID_EXPORT, ID_IMPORT].iter().map(|&i| (i, width(i))).collect();
            ReleaseDC(self.hwnd.get(), hdc);
            let gap = self.s(8);
            let total = left.iter().chain(right.iter()).map(|b| b.1).sum::<i32>() + gap * 6 + gap * 2 + self.s(20);
            (left, right, total)
        }
    }

    fn layout(&self) {
        unsafe {
            let hwnd = self.hwnd.get();
            if hwnd.is_null() || self.ctl.get().list.is_null() {
                return;
            }
            let c = self.ctl.get();
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let (cw, ch) = (rc.right, rc.bottom);
            let (m, gap, row_h, btn_h) = (self.s(10), self.s(8), self.s(28), self.s(30));
            let hdc = GetDC(hwnd);
            let label_w = self.text_width(hdc, "Project (repo URL or owner/repo):") + self.s(4);
            let add_w = self.button_width(hdc, "Add project");
            let flabel_w = self.text_width(hdc, "Filter:") + self.s(4);
            let kind_w = self.button_width(hdc, "Show: Updates available");
            let clear_w = self.button_width(hdc, "Clear");
            let count_w = self.text_width(hdc, "0000 of 0000") + self.s(8);
            ReleaseDC(hwnd, hdc);
            // One batched move for every control: a single repaint pass instead of one per control.
            let mut batch = BeginDeferWindowPos(32);
            let mut mv = |h: HWND, x: i32, y: i32, w: i32, hh: i32| {
                if !batch.is_null() {
                    batch = DeferWindowPos(batch, h, null_mut(), x, y, w.max(1), hh.max(1), SWP_NOZORDER | SWP_NOACTIVATE);
                }
            };
            mv(c.label, m, m, label_w, row_h);
            let add = GetDlgItem(hwnd, ID_ADD);
            mv(add, cw - m - add_w, m, add_w, row_h);
            // The repo box is a rounded frame drawn by the window (see WM_ERASEBKGND); the real edit
            // control sits inside it, borderless, and takes the same field colour.
            let fr = RECT { left: m + label_w + gap, top: m, right: cw - m - add_w - gap, bottom: m + row_h };
            self.entry_frame.set(fr);
            let th = self.s(18).min(row_h - 2);
            mv(c.entry, fr.left + self.s(8), fr.top + (row_h - th) / 2, fr.right - fr.left - 2 * self.s(8), th);
            let y2 = m + row_h + self.s(6);
            let (left, right, _) = self.toolbar_plan();
            let mut x = m;
            for (id, bw) in left {
                mv(GetDlgItem(hwnd, id), x, y2, bw, btn_h);
                x += bw + gap;
            }
            let mut x = cw - m;
            for (id, bw) in right {
                x -= bw;
                mv(GetDlgItem(hwnd, id), x, y2, bw, btn_h);
                x -= gap;
            }
            // second row: search box, status filter, clear, "n of m"
            let y3 = y2 + btn_h + self.s(6);
            mv(c.flabel, m, y3, flabel_w, row_h);
            let mut x = cw - m - count_w;
            mv(c.fcount, x, y3, count_w, row_h);
            x -= gap + clear_w;
            mv(GetDlgItem(hwnd, ID_FCLEAR), x, y3, clear_w, row_h);
            x -= gap + kind_w;
            mv(GetDlgItem(hwnd, ID_FKIND), x, y3, kind_w, row_h);
            let ff = RECT { left: m + flabel_w + gap, top: y3, right: (x - gap).max(m + flabel_w + gap + self.s(40)), bottom: y3 + row_h };
            self.filter_frame.set(ff);
            mv(c.filter, ff.left + self.s(8), ff.top + (row_h - th) / 2, ff.right - ff.left - 2 * self.s(8), th);
            let status_h = self.s(28);
            let top = y3 + row_h + self.s(8);
            mv(c.list, m, top, cw - 2 * m, ch - top - status_h - self.s(2));
            let tsz = self.s(28);
            mv(GetDlgItem(hwnd, ID_THEME), cw - m - tsz, ch - status_h + (status_h - tsz) / 2, tsz, tsz);
            mv(c.status, m, ch - status_h, cw - 2 * m - tsz - gap, status_h);
            if !batch.is_null() {
                EndDeferWindowPos(batch);
            }
            // The rounded repo frame is painted by this window, so the old frame (a different width)
            // must be erased explicitly or stale white pieces stay behind while resizing.
            let strip = RECT { left: 0, top: 0, right: cw, bottom: y3 + row_h + self.s(4) };
            RedrawWindow(hwnd, &strip, null_mut(), RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN);
            // A STATIC with an ellipsis only repaints the newly exposed part when it grows, which
            // garbles the text; repaint the whole control (and its background) every time.
            RedrawWindow(c.status, null(), null_mut(), RDW_INVALIDATE | RDW_ERASE);
            self.apply_columns();
        }
    }

    fn min_track(&self) -> (i32, i32) {
        unsafe {
            let (_, _, tb) = self.toolbar_plan();
            let mut rc = RECT { left: 0, top: 0, right: tb.max(self.s(700)), bottom: self.s(340) };
            AdjustWindowRectExForDpi(&mut rc, WS_OVERLAPPEDWINDOW, 0, 0, self.dpi.get());
            (rc.right - rc.left, rc.bottom - rc.top)
        }
    }

    /// Resize every column in proportion to the window (same idea as the Python version).
    fn apply_columns(&self) {
        unsafe {
            let c = self.ctl.get();
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(c.list, &mut rc);
            let base: Vec<i32> = self.col_base.borrow().iter().map(|&v| self.s(v)).collect();
            let low: Vec<i32> = COL_MIN.iter().map(|&v| self.s(v)).collect();
            let widths = layout_columns(&base, &low, rc.right);
            let changed: Vec<(usize, i32)> = widths
                .iter()
                .enumerate()
                .filter(|(i, wd)| SendMessageW(c.list, LVM_GETCOLUMNWIDTH, *i, 0) as i32 != **wd)
                .map(|(i, wd)| (i, *wd))
                .collect();
            if changed.is_empty() {
                return; // e.g. only the height changed
            }
            SendMessageW(c.list, WM_SETREDRAW, 0, 0);
            for (i, wd) in changed {
                SendMessageW(c.list, LVM_SETCOLUMNWIDTH, i, wd as isize);
            }
            SendMessageW(c.list, WM_SETREDRAW, 1, 0);
            RedrawWindow(c.list, null(), null_mut(), RDW_INVALIDATE | RDW_ALLCHILDREN); // paint once, later
        }
    }

    /// The user dragged a column border: the new widths become the preferred ones, and the freed
    /// space goes to the last column so no empty strip is left on the right.
    fn after_track(&self) {
        unsafe {
            let c = self.ctl.get();
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(c.list, &mut rc);
            let low: Vec<i32> = COL_MIN.iter().map(|&v| self.s(v)).collect();
            let mut cur: Vec<i32> = (0..COLS.len()).map(|i| (SendMessageW(c.list, LVM_GETCOLUMNWIDTH, i, 0) as i32).max(low[i])).collect();
            let slack = rc.right - cur.iter().sum::<i32>();
            if slack > 0 {
                *cur.last_mut().unwrap() += slack;
            }
            *self.col_base.borrow_mut() = cur.iter().map(|&v| v * 96 / self.dpi.get() as i32).collect();
            for (i, wd) in cur.iter().enumerate() {
                SendMessageW(c.list, LVM_SETCOLUMNWIDTH, i, *wd as isize);
            }
            InvalidateRect(c.list, null(), 1);
        }
    }

    // ------------------------------------------------------------------ list
    fn selected_keys(&self) -> Vec<String> {
        let c = self.ctl.get();
        let (data, view) = (self.data.borrow(), self.view.borrow());
        let mut out = Vec::new();
        let mut i = -1isize;
        unsafe {
            loop {
                i = SendMessageW(c.list, LVM_GETNEXTITEM, i as usize, LVNI_SELECTED as isize);
                if i < 0 {
                    break;
                }
                if let Some(p) = view.get(i as usize).and_then(|&ix| data.projects.get(ix)) {
                    out.push(project_key(p));
                }
            }
        }
        out
    }

    fn with_selected<R>(&self, f: impl FnOnce(&mut Vec<&mut Project>) -> R) -> R {
        let keys = self.selected_keys();
        let mut data = self.data.borrow_mut();
        let mut sel: Vec<&mut Project> = data.projects.iter_mut().filter(|p| keys.contains(&project_key(p))).collect();
        f(&mut sel)
    }

    /// Call after projects were removed or the whole list was replaced: the old row numbers no
    /// longer match the data, so nothing may be re-selected from them.
    fn forget_view(&self) {
        self.view.borrow_mut().clear();
    }

    /// What is typed in the search box.
    fn filter_query(&self) -> String {
        unsafe {
            let e = self.ctl.get().filter;
            let n = GetWindowTextLengthW(e) as usize;
            let mut buf = vec![0u16; n + 2];
            GetWindowTextW(e, buf.as_mut_ptr(), buf.len() as i32);
            from_wide_buf(&buf)
        }
    }

    fn filter_active(&self) -> bool {
        !self.filter_query().trim().is_empty() || *self.filter_kind.borrow() != "all"
    }

    fn set_kind(&self, kind: &str) {
        *self.filter_kind.borrow_mut() = kind.to_string();
        unsafe { InvalidateRect(GetDlgItem(self.hwnd.get(), ID_FKIND), null(), 1) };
        self.render();
    }

    fn clear_filter(&self) {
        unsafe {
            SetWindowTextW(self.ctl.get().filter, w("").as_ptr());
            KillTimer(self.hwnd.get(), TIMER_FILTER);
            InvalidateRect(GetDlgItem(self.hwnd.get(), ID_FKIND), null(), 1);
        }
        *self.filter_kind.borrow_mut() = "all".to_string();
        self.render();
    }

    fn kind_menu(&self) {
        unsafe {
            let menu = CreatePopupMenu();
            let cur = self.filter_kind.borrow().clone();
            for (i, (value, text)) in STATUS_FILTERS.iter().enumerate() {
                AppendMenuW(menu, MF_STRING | if *value == cur { MF_CHECKED } else { 0 }, M_KIND as usize + i + 1, w(text).as_ptr());
            }
            let mut rc: RECT = std::mem::zeroed();
            GetWindowRect(GetDlgItem(self.hwnd.get(), ID_FKIND), &mut rc);
            let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, rc.left, rc.bottom, 0, self.hwnd.get(), null());
            DestroyMenu(menu);
            self.run_menu_command(cmd as u32);
        }
    }

    pub fn render(&self) {
        unsafe {
            let c = self.ctl.get();
            let keep = self.selected_keys();
            self.suppress.set(true);
            {
                let data = self.data.borrow();
                let shown = filter_matches(&data.projects, &self.filter_query(), &self.filter_kind.borrow());
                let order = sort_order(&data.projects, &self.sort_col.borrow(), self.sort_asc.get());
                let count = shown.iter().filter(|&&b| b).count();
                let total = data.projects.len();
                let text = if count != total { format!("{} of {}", count, total) } else { format!("{} project{}", total, if total == 1 { "" } else { "s" }) };
                SetWindowTextW(c.fcount, w(&text).as_ptr());
                *self.view.borrow_mut() = order.into_iter().filter(|&i| shown[i]).collect();
            }
            SendMessageW(c.list, LVM_SETITEMCOUNT, self.view.borrow().len(), LVSICF_NOSCROLL as isize);
            let mut st: LVITEMW = std::mem::zeroed();
            st.stateMask = LVIS_SELECTED;
            st.state = 0;
            SendMessageW(c.list, LVM_SETITEMSTATE, usize::MAX, &st as *const LVITEMW as isize);
            st.state = LVIS_SELECTED;
            {
                let (data, view) = (self.data.borrow(), self.view.borrow());
                for (row, &ix) in view.iter().enumerate() {
                    if keep.contains(&project_key(&data.projects[ix])) {
                        SendMessageW(c.list, LVM_SETITEMSTATE, row, &st as *const LVITEMW as isize);
                    }
                }
            }
            InvalidateRect(c.list, null(), 1);
            self.suppress.set(false);
            self.apply_columns();
        }
    }

    fn cell_text(&self, row: usize, col: usize) -> String {
        let (data, view) = (self.data.borrow(), self.view.borrow());
        let p = match view.get(row).and_then(|&i| data.projects.get(i)) {
            Some(p) => p,
            None => return String::new(),
        };
        match col {
            0 => display_name(p),
            1 => repo_path(p),
            2 => source_label(p),
            3 => release_label(p),
            4 => published_label(p),
            5 => status_text(p),
            _ => checked_label(p),
        }
    }

    fn update_headings(&self) {
        unsafe {
            let c = self.ctl.get();
            let col = self.sort_col.borrow().clone();
            for (i, (id, _)) in COLS.iter().enumerate() {
                let mut item: HDITEMW = std::mem::zeroed();
                item.mask = HDI_FORMAT;
                SendMessageW(c.header, HDM_GETITEMW, i, &mut item as *mut HDITEMW as isize);
                item.fmt &= !(HDF_SORTUP | HDF_SORTDOWN);
                item.fmt |= HDF_LEFT;
                if *id == col {
                    item.fmt |= if self.sort_asc.get() { HDF_SORTUP } else { HDF_SORTDOWN };
                }
                SendMessageW(c.header, HDM_SETITEMW, i, &item as *const HDITEMW as isize);
            }
        }
    }

    fn sort_by(&self, col: usize) {
        let id = COLS[col].0;
        if !SORTABLE.contains(&id) {
            return;
        }
        if *self.sort_col.borrow() == id {
            self.sort_asc.set(!self.sort_asc.get());
        } else {
            *self.sort_col.borrow_mut() = id.to_string();
            self.sort_asc.set(id != "published"); // A-Z for names, newest first for dates
        }
        {
            let mut d = self.data.borrow_mut();
            d.settings.sort_col = id.to_string();
            d.settings.sort_asc = self.sort_asc.get();
        }
        self.persist();
        self.update_headings();
        self.render();
    }

    // ------------------------------------------------------------------ status / persistence
    pub fn set_status(&self, text: &str) {
        *self.status_text.borrow_mut() = text.to_string();
        unsafe {
            SetWindowTextW(self.ctl.get().status, w(text).as_ptr());
        }
    }

    pub fn persist(&self) {
        let text = data_to_json(&self.data.borrow());
        if let Err(e) = write_atomic(&self.data_path, &text) {
            message_box(self.hwnd.get(), &format!("Could not save projects.json:\n{}", e), MB_OK | MB_ICONERROR);
        }
    }

    fn token(&self) -> String {
        self.data.borrow().settings.token.trim().to_string()
    }

    fn on_select(&self) {
        if self.suppress.get() {
            return;
        }
        let keys = self.selected_keys();
        if keys.len() == 1 {
            let text = {
                let d = self.data.borrow();
                d.projects.iter().find(|p| project_key(p) == keys[0]).map(|p| match &p.error {
                    Some(e) => e.clone(),
                    None if p.description.is_empty() => NO_DESCRIPTION.to_string(),
                    None => p.description.clone(),
                })
            };
            if let Some(t) = text {
                self.set_status(&t);
            }
        }
    }

    fn set_busy(&self, busy: bool, text: Option<&str>) {
        self.busy.set(busy);
        unsafe {
            for id in [ID_ADD, ID_CHECK] {
                EnableWindow(GetDlgItem(self.hwnd.get(), id), (!busy) as i32);
            }
        }
        if let Some(t) = text {
            self.set_status(t);
        }
    }

    // ------------------------------------------------------------------ actions
    fn add_project(&self) {
        if self.busy.get() {
            return;
        }
        let hwnd = self.hwnd.get();
        let text = unsafe {
            let e = self.ctl.get().entry;
            let n = GetWindowTextLengthW(e) as usize;
            let mut buf = vec![0u16; n + 2];
            GetWindowTextW(e, buf.as_mut_ptr(), buf.len() as i32);
            from_wide_buf(&buf)
        };
        let pref = match parse_project_ref(&text) {
            Some(x) => x,
            None => {
                message_box(
                    hwnd,
                    "Enter a project URL such as\nhttps://github.com/owner/repo,\nhttps://codeberg.org/owner/repo or https://gitlab.com/group/project\n(any GitLab, Gitea or Forgejo server works), or just owner/repo for GitHub.",
                    MB_OK | MB_ICONERROR,
                );
                return;
            }
        };
        let (owner, repo) = split_path(&pref.segs);
        let shown = if pref.host.is_empty() { format!("{}/{}", owner, repo) } else { format!("{}/{}/{}", host_label(&pref.host), owner, repo) };
        if self.data.borrow().projects.iter().any(|p| project_key(p) == shown.to_lowercase()) {
            message_box(hwnd, &format!("{} is already in the list.", shown), MB_OK | MB_ICONINFORMATION);
            return;
        }
        if self.data.borrow().projects.len() >= MAX_PROJECTS {
            message_box(hwnd, &format!("The list is full ({} projects).", MAX_PROJECTS), MB_OK | MB_ICONINFORMATION);
            return;
        }
        let token = self.token();
        self.set_busy(true, Some(&format!("Adding {} ...", shown)));
        let net = self.net.clone();
        let target = hwnd as usize;
        std::thread::spawn(move || {
            if !pref.host.is_empty() {
                // another host: public repositories only, official releases only
                let result = resolve_project(&*net, &pref.host, pref.kind.as_deref(), &pref.segs, true).map_err(|e| e.to_string());
                post_job(target, Job::Added(result.map(|(info, latest)| (info, Some(latest)))));
                return;
            }
            let result = (|| {
                let latest = get_latest(&*net, &owner, &repo, &token, true).map_err(|e| e.to_string());
                let info = match fetch_repo_info(&*net, &owner, &repo, &token) {
                    Ok(i) => i,
                    Err(GitHubError::NotFound(m)) => return Err(m),
                    // API unavailable (e.g. rate-limited): the release feed is enough.
                    Err(_) => RepoInfo { owner: owner.clone(), repo: repo.clone(), title: repo.clone(), description: String::new(), html_url: format!("https://github.com/{}/{}", owner, repo), host: None, kind: None },
                };
                Ok((info, latest?))
            })();
            post_job(target, Job::Added(result));
        });
    }

    fn check_updates(&self) {
        if self.busy.get() {
            return;
        }
        let snapshot: Vec<(String, Option<String>, String, String, bool, String)> = self
            .data
            .borrow()
            .projects
            .iter()
            .map(|p| (p.host.clone().unwrap_or_default(), p.kind.clone(), p.owner.clone(), p.repo.clone(), p.include_prereleases, project_key(p)))
            .collect();
        if snapshot.is_empty() {
            self.set_status("Nothing to check - add a project first.");
            return;
        }
        let token = self.token();
        self.set_busy(true, Some(&format!("Checking {} project(s) ...", snapshot.len())));
        let net = self.net.clone();
        let target = self.hwnd.get() as usize;
        std::thread::spawn(move || {
            let mut results = Vec::new();
            for (host, kind, owner, repo, inc, key) in snapshot {
                match fetch_latest_for(&*net, &host, kind.as_deref(), &owner, &repo, &token, inc) {
                    Ok(r) => results.push((key, r, None)),
                    Err(GitHubError::RateLimit(m)) => {
                        results.push((key, None, Some(m)));
                        break; // no point continuing
                    }
                    Err(e) => results.push((key, None, Some(e.to_string()))),
                }
            }
            post_job(target, Job::Checked { results, rate: net.rate_remaining() });
        });
    }

    fn on_job(&self, job: Job) {
        self.set_busy(false, None);
        match job {
            Job::Added(Err(e)) => {
                self.set_status("Could not add project.");
                message_box(self.hwnd.get(), &e, MB_OK | MB_ICONERROR);
            }
            Job::Added(Ok((info, latest))) => {
                let mut p = Project {
                    owner: info.owner, repo: info.repo, host: info.host, kind: info.kind, title: info.title, description: info.description, html_url: info.html_url,
                    include_prereleases: true, // "whatever is newest", as on the Releases page
                    fmt: Some(2),
                    // Baseline: the release that exists now counts as already seen.
                    seen_id: latest.as_ref().map(|l| l.id.clone()),
                    seen_tag: latest.as_ref().map(|l| l.tag.clone()),
                    latest,
                    last_checked: now_iso(),
                    error: None,
                    custom_name: None,
                };
                // keep the same cleaning as imports (long titles etc.)
                if let Some(n) = normalize_project(&serde_json::to_value(&p).unwrap_or_default()) {
                    p = n;
                }
                if self.data.borrow().projects.iter().any(|x| project_key(x) == project_key(&p)) {
                    self.set_status("Already in the list.");
                    return;
                }
                let msg = match &p.latest {
                    Some(l) => format!("Added {} - current release {}.", p.title, if l.tag.is_empty() { &l.name } else { &l.tag }),
                    None => format!("Added {} - it has no published releases yet.", p.title),
                };
                self.data.borrow_mut().projects.push(p);
                self.persist();
                unsafe { SetWindowTextW(self.ctl.get().entry, w("").as_ptr()) };
                if self.filter_active() {
                    self.clear_filter(); // a filtered-out new project would look like it was not added
                } else {
                    self.render();
                }
                self.set_status(&msg);
            }
            Job::Checked { results, rate } => {
                let now = now_iso();
                let mut errors = 0;
                {
                    let mut data = self.data.borrow_mut();
                    for (key, latest, err) in &results {
                        if let Some(p) = data.projects.iter_mut().find(|p| project_key(p) == *key) {
                            if err.is_some() {
                                errors += 1;
                            }
                            apply_result(p, latest.clone(), err.clone(), &now);
                        }
                    }
                }
                self.persist();
                self.render();
                let updates = self.data.borrow().projects.iter().filter(|p| has_update(p)).count();
                let mut msg = format!("Checked {} project(s): {} update(s) available", results.len(), updates);
                if errors > 0 {
                    msg.push_str(&format!(", {} error(s)", errors));
                }
                if let Some(r) = rate {
                    msg.push_str(&format!(". API calls left this hour: {}", r));
                }
                self.set_status(&(msg + "."));
            }
        }
    }

    fn mark_seen(&self) {
        if self.selected_keys().is_empty() {
            self.set_status("Select one or more projects first.");
            return;
        }
        let n = self.with_selected(|sel| {
            let mut n = 0;
            for p in sel.iter_mut() {
                if has_update(p) {
                    if let Some(l) = p.latest.clone() {
                        p.seen_id = Some(l.id);
                        p.seen_tag = Some(l.tag);
                        n += 1;
                    }
                }
            }
            n
        });
        self.persist();
        self.render();
        self.set_status(&if n > 0 { format!("Marked {} project(s) as seen.", n) } else { "No updates to mark in the selection.".to_string() });
    }

    fn first_selected(&self) -> Option<Project> {
        let keys = self.selected_keys();
        let first = keys.first()?.clone();
        self.data.borrow().projects.iter().find(|p| project_key(p) == first).cloned()
    }

    fn open_release(&self) {
        match self.first_selected() {
            None => self.set_status("Select a project first."),
            Some(p) => {
                shell_open(&safe_url(p.latest.as_ref().map(|l| l.url.as_str()), &project_base(&p), &release_page(&p)));
            }
        }
    }

    fn open_repo(&self) {
        if let Some(p) = self.first_selected() {
            shell_open(&safe_url(Some(&p.html_url), &project_base(&p), &repo_page(&p)));
        }
    }

    fn toggle_prerelease(&self) {
        let include = self.with_selected(|sel| {
            let new = !sel.first().map_or(true, |p| p.include_prereleases);
            for p in sel.iter_mut() {
                p.include_prereleases = new;
            }
            new
        });
        self.persist();
        self.set_status(if include {
            "Pre-releases included (uses the free release feed). Click \"Check for updates\" to apply."
        } else {
            "Stable-only mode uses the GitHub API, which is rate-limited without a token. Click \"Check for updates\" to apply."
        });
    }

    fn remove_selected(&self) {
        let keys = self.selected_keys();
        if keys.is_empty() {
            self.set_status("Select a project first.");
            return;
        }
        let names = {
            let d = self.data.borrow();
            let list: Vec<String> = d.projects.iter().filter(|p| keys.contains(&project_key(p))).map(display_name).collect();
            list.iter().take(3).cloned().collect::<Vec<_>>().join(", ") + if list.len() > 3 { "..." } else { "" }
        };
        if message_box(self.hwnd.get(), &format!("Remove {} project(s)?\n{}", keys.len(), names), MB_YESNO | MB_ICONQUESTION) != IDYES {
            return;
        }
        self.data.borrow_mut().projects.retain(|p| !keys.contains(&project_key(p)));
        self.forget_view();
        self.persist();
        self.render();
        self.set_status(&format!("Removed {} project(s).", keys.len()));
    }

    fn rename_selected(&self) {
        unsafe {
            let c = self.ctl.get();
            let i = SendMessageW(c.list, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as isize);
            if self.selected_keys().len() != 1 || i < 0 {
                self.set_status("Select one project to rename.");
                return;
            }
            self.hide_pencil();
            SetFocus(c.list);
            SendMessageW(c.list, LVM_EDITLABELW, i as usize, 0);
        }
    }

    fn reset_names(&self) {
        let n = self.with_selected(|sel| {
            let mut n = 0;
            for p in sel.iter_mut() {
                if p.custom_name.take().is_some() {
                    n += 1;
                }
            }
            n
        });
        if n == 0 {
            self.set_status("Nothing to reset: the selection has no custom name.");
            return;
        }
        self.persist();
        self.render();
        self.set_status(&format!("Reset {} name(s) to the GitHub name.", n));
    }

    fn import_json(&self) {
        let hwnd = self.hwnd.get();
        let path = match open_file_dialog(hwnd) {
            Some(p) => p,
            None => return,
        };
        let raw: serde_json::Value = match std::fs::metadata(&path) {
            Ok(m) if m.len() > MAX_IMPORT_BYTES => {
                message_box(hwnd, "Could not read that file as JSON:\nThe file is larger than 5 MB.", MB_OK | MB_ICONERROR);
                return;
            }
            Err(e) => {
                message_box(hwnd, &format!("Could not read that file as JSON:\n{}", e), MB_OK | MB_ICONERROR);
                return;
            }
            Ok(_) => match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())) {
                Ok(v) => v,
                Err(e) => {
                    message_box(hwnd, &format!("Could not read that file as JSON:\n{}", e), MB_OK | MB_ICONERROR);
                    return;
                }
            },
        };
        let (projects, skipped) = match parse_import(&raw) {
            None => {
                message_box(hwnd, "This file has no project list.", MB_OK | MB_ICONERROR);
                return;
            }
            Some((p, _)) if p.is_empty() => {
                message_box(hwnd, "No valid projects found in this file.", MB_OK | MB_ICONERROR);
                return;
            }
            Some(x) => x,
        };
        let note = if skipped > 0 { format!(" ({} entries skipped as invalid or duplicate)", skipped) } else { String::new() };
        let choice = message_box(
            hwnd,
            &format!(
                "Found {} project(s){}.\n\nYes = Merge: keep your current list and add the new ones\nNo = Replace: discard your current list and use this file\nCancel = do nothing",
                projects.len(), note
            ),
            MB_YESNOCANCEL | MB_ICONQUESTION,
        );
        let msg = match choice {
            IDYES => {
                let total = projects.len();
                let added = merge_projects(&mut self.data.borrow_mut().projects, projects);
                format!("Imported: {} new project(s) added, {} already in the list.", added, total - added)
            }
            IDNO => {
                let n = projects.len();
                self.data.borrow_mut().projects = projects;
                self.forget_view();
                format!("Imported: list replaced with {} project(s).", n)
            }
            _ => return,
        };
        self.persist();
        self.render();
        self.set_status(&format!("{} Click \"Check for updates\" to refresh them.", msg));
    }

    fn export_json(&self) {
        let hwnd = self.hwnd.get();
        if self.data.borrow().projects.is_empty() {
            message_box(hwnd, "There are no projects to export yet.", MB_OK | MB_ICONINFORMATION);
            return;
        }
        let path = match save_file_dialog(hwnd, &format!("github_release_tracker_{}.json", today_ymd())) {
            Some(p) => p,
            None => return,
        };
        let payload = build_export(&self.data.borrow().projects, &now_iso());
        let text = serde_json::to_string_pretty(&payload).unwrap_or_default();
        match write_atomic(std::path::Path::new(&path), &text) {
            Ok(()) => self.set_status(&format!("Exported {} project(s) to {} (token not included).", self.data.borrow().projects.len(), path)),
            Err(e) => {
                message_box(hwnd, &format!("Could not write the file:\n{}", e), MB_OK | MB_ICONERROR);
            }
        }
    }

    fn popup_menu(&self, x: i32, y: i32) {
        unsafe {
            let sel = self.with_selected(|s| s.iter().map(|p| (p.include_prereleases, p.custom_name.is_some())).collect::<Vec<_>>());
            if sel.is_empty() {
                return;
            }
            let menu = CreatePopupMenu();
            let add = |id: u32, text: &str, flags: u32| {
                AppendMenuW(menu, MF_STRING | flags, id as usize, w(text).as_ptr());
            };
            add(M_OPEN, "Open release page", 0);
            add(M_REPO, "Open repository", 0);
            add(M_SEEN, "Mark as seen", 0);
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            add(M_RENAME, "Rename...\tF2", if sel.len() == 1 { 0 } else { MF_GRAYED });
            add(M_RESET, "Reset name", if sel.iter().any(|s| s.1) { 0 } else { MF_GRAYED });
            add(M_PRE, "Include pre-releases", if sel[0].0 { MF_CHECKED } else { 0 });
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            add(M_REMOVE, "Remove", 0);
            let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, x, y, 0, self.hwnd.get(), null());
            DestroyMenu(menu);
            self.run_menu_command(cmd as u32);
        }
    }

    fn run_menu_command(&self, cmd: u32) {
        match cmd {
            M_OPEN => self.open_release(),
            M_REPO => self.open_repo(),
            M_SEEN => self.mark_seen(),
            M_RENAME => self.rename_selected(),
            M_RESET => self.reset_names(),
            M_PRE => self.toggle_prerelease(),
            M_REMOVE => self.remove_selected(),
            n if n > M_KIND && (n - M_KIND) as usize <= STATUS_FILTERS.len() => self.set_kind(STATUS_FILTERS[(n - M_KIND - 1) as usize].0),
            M_COPY => {
                copy_to_clipboard(self.hwnd.get(), &self.status_text.borrow());
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------ drawing
    unsafe fn draw_button(&self, dis: &DRAWITEMSTRUCT) {
        let p = self.pal();
        let id = dis.CtlID as i32;
        if id == ID_PENCIL {
            let (bg, fg) = self.row_colors(self.pencil_row.get());
            let br = CreateSolidBrush(bg);
            FillRect(dis.hDC, &dis.rcItem, br);
            DeleteObject(br as HGDIOBJ);
            SetBkMode(dis.hDC, TRANSPARENT as i32);
            SetTextColor(dis.hDC, fg);
            let old = SelectObject(dis.hDC, self.fonts.get().icon as HGDIOBJ);
            let g = [G_EDIT, 0u16];
            let mut r = dis.rcItem;
            DrawTextW(dis.hDC, g.as_ptr(), 1, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
            SelectObject(dis.hDC, old);
            return;
        }
        let (_, glyph, text) = match BUTTONS.iter().find(|b| b.0 == id) {
            Some(b) => *b,
            None => return,
        };
        let kind_text;
        let text: &str = if id == ID_FKIND {
            kind_text = format!("Show: {}", status_filter_text(&self.filter_kind.borrow()));
            &kind_text
        } else {
            text
        };
        let glyph = if id == ID_THEME { if self.dark.get() { G_SUN } else { G_MOON } } else { glyph };
        let disabled = dis.itemState & ODS_DISABLED != 0;
        let pressed = dis.itemState & ODS_SELECTED != 0;
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let flat = id == ID_THEME; // icon only, no box
        // corners first get the window background, then the rounded box (Windows 11 look)
        let back = CreateSolidBrush(p.bg);
        FillRect(hdc, &rc, back);
        DeleteObject(back as HGDIOBJ);
        if flat {
            if pressed {
                let fill = CreateSolidBrush(p.btn_active);
                let (ob, op) = (SelectObject(hdc, fill as HGDIOBJ), SelectObject(hdc, GetStockObject(NULL_PEN)));
                let e = self.s(8);
                RoundRect(hdc, rc.left, rc.top, rc.right + 1, rc.bottom + 1, e, e);
                SelectObject(hdc, ob);
                SelectObject(hdc, op);
                DeleteObject(fill as HGDIOBJ);
            }
        } else {
            let fill = CreateSolidBrush(if pressed { p.btn_active } else { p.btn_bg });
            let pen = CreatePen(PS_SOLID, 1, if dis.itemState & ODS_FOCUS != 0 { p.sel_bg } else { p.border });
            let (ob, op) = (SelectObject(hdc, fill as HGDIOBJ), SelectObject(hdc, pen as HGDIOBJ));
            let e = self.s(8);
            RoundRect(hdc, rc.left, rc.top, rc.right, rc.bottom, e, e);
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
            DeleteObject(fill as HGDIOBJ);
            DeleteObject(pen as HGDIOBJ);
        }
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, if disabled { p.none_fg } else { p.fg });
        let f = self.fonts.get();
        let icon_w = self.s(16);
        let gap = if text.is_empty() { 0 } else { self.s(6) };
        let old = SelectObject(hdc, f.ui as HGDIOBJ);
        let tw = if text.is_empty() { 0 } else { self.text_width(hdc, text) };
        let total = icon_w + gap + tw;
        let x0 = rc.left + ((rc.right - rc.left) - total) / 2;
        let mut ir = RECT { left: x0, top: rc.top, right: x0 + icon_w, bottom: rc.bottom };
        SelectObject(hdc, f.icon as HGDIOBJ);
        let g = [glyph, 0u16];
        DrawTextW(hdc, g.as_ptr(), 1, &mut ir, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        if !text.is_empty() {
            SelectObject(hdc, f.ui as HGDIOBJ);
            let mut tr = RECT { left: x0 + icon_w + gap, top: rc.top, right: rc.right, bottom: rc.bottom };
            let t = w(text);
            DrawTextW(hdc, t.as_ptr(), -1, &mut tr, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        }
        SelectObject(hdc, old);
    }

    /// (background, text) colours of a list row, selection included.
    fn row_colors(&self, row: isize) -> (u32, u32) {
        let p = self.pal();
        let c = self.ctl.get();
        if row < 0 {
            return (p.field, p.fg);
        }
        unsafe {
            if SendMessageW(c.list, LVM_GETITEMSTATE, row as usize, LVIS_SELECTED as isize) as u32 & LVIS_SELECTED != 0 {
                return (p.sel_bg, p.sel_fg);
            }
        }
        let (data, view) = (self.data.borrow(), self.view.borrow());
        match view.get(row as usize).and_then(|&i| data.projects.get(i)) {
            Some(pr) if pr.error.is_some() => (p.field, p.error_fg),
            Some(pr) if has_update(pr) => (p.update_bg, p.update_fg),
            Some(pr) if pr.latest.is_none() => (p.field, p.none_fg),
            _ => (p.field, p.fg),
        }
    }

    /// One header cell, drawn by us so dark and light mode get the right colours.
    unsafe fn header_item(&self, hdc: HDC, rc: RECT, i: usize, hot: bool) {
        let p = self.pal();
        let c = self.ctl.get();
        let br = CreateSolidBrush(if hot { p.btn_bg } else { p.head_bg });
        FillRect(hdc, &rc, br);
        DeleteObject(br as HGDIOBJ);
        let ln = CreateSolidBrush(p.line);
        FillRect(hdc, &RECT { left: rc.right - 1, top: rc.top + 2, right: rc.right, bottom: rc.bottom - 2 }, ln);
        FillRect(hdc, &RECT { left: rc.left, top: rc.bottom - 1, right: rc.right, bottom: rc.bottom }, ln);
        DeleteObject(ln as HGDIOBJ);
        let mut buf = [0u16; 64];
        let mut item: HDITEMW = std::mem::zeroed();
        item.mask = HDI_TEXT;
        item.pszText = buf.as_mut_ptr();
        item.cchTextMax = 63;
        SendMessageW(c.header, HDM_GETITEMW, i, &mut item as *mut HDITEMW as isize);
        let sorted = COLS.get(i).map_or(false, |col| *self.sort_col.borrow() == col.0);
        let old = SelectObject(hdc, self.fonts.get().bold as HGDIOBJ);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, p.fg);
        let mut tr = RECT { left: rc.left + self.s(6), top: rc.top, right: rc.right - self.s(if sorted { 20 } else { 6 }), bottom: rc.bottom };
        DrawTextW(hdc, buf.as_ptr(), -1, &mut tr, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX);
        SelectObject(hdc, old);
        if sorted {
            let (cx, cy, h) = (rc.right - self.s(12), (rc.top + rc.bottom) / 2, self.s(3));
            let pts = if self.sort_asc.get() {
                [POINT { x: cx - h - 1, y: cy + h / 2 + 1 }, POINT { x: cx + h + 1, y: cy + h / 2 + 1 }, POINT { x: cx, y: cy - h }]
            } else {
                [POINT { x: cx - h - 1, y: cy - h / 2 - 1 }, POINT { x: cx + h + 1, y: cy - h / 2 - 1 }, POINT { x: cx, y: cy + h }]
            };
            let ab = CreateSolidBrush(p.fg);
            let ap = CreatePen(PS_SOLID, 1, p.fg);
            let (ob, op) = (SelectObject(hdc, ab as HGDIOBJ), SelectObject(hdc, ap as HGDIOBJ));
            Polygon(hdc, pts.as_ptr(), 3);
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
            DeleteObject(ab as HGDIOBJ);
            DeleteObject(ap as HGDIOBJ);
        }
    }

    /// WM_PAINT of the header (called from its subclass): the whole header is painted here, so the
    /// system never draws its own white/black version.
    unsafe fn header_paint(&self, hwnd: HWND) {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut cr: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut cr);
        let mem = CreateCompatibleDC(hdc);
        let bmp = CreateCompatibleBitmap(hdc, cr.right.max(1), cr.bottom.max(1));
        let old_bmp = SelectObject(mem, bmp as HGDIOBJ);
        let bg = CreateSolidBrush(self.pal().head_bg);
        FillRect(mem, &cr, bg);
        DeleteObject(bg as HGDIOBJ);
        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);
        ScreenToClient(hwnd, &mut pt);
        let mut hti: HDHITTESTINFO = std::mem::zeroed();
        hti.pt = pt;
        let hot = SendMessageW(hwnd, HDM_HITTEST, 0, &mut hti as *mut HDHITTESTINFO as isize);
        let hot = if hti.flags & HHT_ONHEADER != 0 { hot } else { -1 };
        let n = SendMessageW(hwnd, HDM_GETITEMCOUNT, 0, 0).max(0) as usize;
        for i in 0..n {
            let mut r: RECT = std::mem::zeroed();
            if SendMessageW(hwnd, HDM_GETITEMRECT, i, &mut r as *mut RECT as isize) != 0 {
                self.header_item(mem, r, i, hot == i as isize);
            }
        }
        BitBlt(hdc, 0, 0, cr.right, cr.bottom, mem, 0, 0, SRCCOPY);
        SelectObject(mem, old_bmp);
        DeleteObject(bmp as HGDIOBJ);
        DeleteDC(mem);
        EndPaint(hwnd, &ps);
    }

    // ---- hover pencil ----
    fn hide_pencil(&self) {
        unsafe {
            let pc = self.pencil.get();
            if !pc.is_null() && IsWindowVisible(pc) != 0 {
                ShowWindow(pc, SW_HIDE);
            }
            self.pencil_row.set(-1);
            KillTimer(self.hwnd.get(), TIMER_PENCIL);
        }
    }

    /// Mouse moved over the list (called from the list's subclass).
    unsafe fn list_mouse_move(&self, x: i32, y: i32) {
        let c = self.ctl.get();
        if GetFocus() != c.list && !GetFocus().is_null() && GetParent(GetFocus()) == c.list {
            return; // a rename box is open
        }
        let mut hti: LVHITTESTINFO = std::mem::zeroed();
        hti.pt = POINT { x, y };
        let row = SendMessageW(c.list, LVM_HITTEST, 0, &mut hti as *mut LVHITTESTINFO as isize);
        if row < 0 || row as usize >= self.view.borrow().len() {
            return; // the timer hides the pencil once the mouse is really gone
        }
        if row == self.pencil_row.get() && IsWindowVisible(self.pencil.get()) != 0 {
            return;
        }
        let mut r: RECT = std::mem::zeroed();
        r.left = LVIR_BOUNDS as i32;
        r.top = 0;
        SendMessageW(c.list, LVM_GETSUBITEMRECT, row as usize, &mut r as *mut RECT as isize);
        let col_right = SendMessageW(c.list, LVM_GETCOLUMNWIDTH, 0, 0) as i32 - GetScrollPos(c.list, SB_HORZ);
        let size = self.s(20);
        let mut hr: RECT = std::mem::zeroed();
        GetWindowRect(c.header, &mut hr);
        if r.top < hr.bottom - hr.top {
            return; // under the header
        }
        let mut pt = POINT { x: (col_right - size - self.s(4)).max(0), y: r.top + ((r.bottom - r.top) - size) / 2 };
        MapWindowPoints(c.list, self.hwnd.get(), &mut pt, 1);
        self.pencil_row.set(row);
        SetWindowPos(self.pencil.get(), HWND_TOP, pt.x, pt.y, size, size, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        RedrawWindow(self.pencil.get(), null(), null_mut(), RDW_INVALIDATE | RDW_UPDATENOW);
        SetTimer(self.hwnd.get(), TIMER_PENCIL, 120, None);
    }

    fn pencil_tick(&self) {
        unsafe {
            let mut pt: POINT = std::mem::zeroed();
            GetCursorPos(&mut pt);
            let (c, pc) = (self.ctl.get(), self.pencil.get());
            let mut lr: RECT = std::mem::zeroed();
            let mut pr: RECT = std::mem::zeroed();
            GetWindowRect(c.list, &mut lr);
            GetWindowRect(pc, &mut pr);
            let inside = |r: &RECT| pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom;
            if !(inside(&lr) || inside(&pr)) || WindowFromPoint(pt).is_null() {
                self.hide_pencil();
            }
        }
    }

    fn pencil_clicked(&self) {
        let row = self.pencil_row.get();
        self.hide_pencil();
        self.rename_row(row);
    }

    /// Select just this row and open the inline name editor on it.
    fn rename_row(&self, row: isize) {
        if row >= 0 {
            unsafe {
                let c = self.ctl.get();
                let mut one: LVITEMW = std::mem::zeroed();
                one.stateMask = LVIS_SELECTED;
                SendMessageW(c.list, LVM_SETITEMSTATE, usize::MAX, &one as *const LVITEMW as isize);
                one.state = LVIS_SELECTED | LVIS_FOCUSED;
                one.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
                SendMessageW(c.list, LVM_SETITEMSTATE, row as usize, &one as *const LVITEMW as isize);
                SetFocus(c.list);
                SendMessageW(c.list, LVM_EDITLABELW, row as usize, 0);
            }
        }
    }

    unsafe fn custom_draw(&self, cd: *mut NMLVCUSTOMDRAW) -> isize {
        let cd = &mut *cd;
        let p = self.pal();
        let c = self.ctl.get();
        match cd.nmcd.dwDrawStage {
            CDDS_PREPAINT => (CDRF_NOTIFYITEMDRAW | CDRF_NOTIFYPOSTPAINT) as isize,
            CDDS_ITEMPREPAINT => {
                // Each row is drawn completely here (background, selection, text), so the colours
                // are exactly ours, the selection turns blue at once and no theme animation or
                // pale theme colour can get in the way.
                let row = cd.nmcd.dwItemSpec;
                let (bg, fg) = self.row_colors(row as isize);
                let hdc = cd.nmcd.hdc;
                let mut ir: RECT = std::mem::zeroed();
                ir.left = LVIR_BOUNDS as i32;
                if SendMessageW(c.list, LVM_GETITEMRECT, row, &mut ir as *mut RECT as isize) == 0 {
                    return CDRF_DODEFAULT as isize;
                }
                let mut cr: RECT = std::mem::zeroed();
                GetClientRect(c.list, &mut cr);
                let br = CreateSolidBrush(bg);
                FillRect(hdc, &RECT { left: 0, top: ir.top, right: cr.right, bottom: ir.bottom }, br);
                DeleteObject(br as HGDIOBJ);
                let old = SelectObject(hdc, self.fonts.get().ui as HGDIOBJ);
                SetBkMode(hdc, TRANSPARENT as i32);
                SetTextColor(hdc, fg);
                let pad = self.s(6);
                let mut x = -GetScrollPos(c.list, SB_HORZ);
                for col in 0..COLS.len() {
                    let wd = SendMessageW(c.list, LVM_GETCOLUMNWIDTH, col, 0) as i32;
                    if x + wd > 0 && x < cr.right {
                        let t = w(&self.cell_text(row, col));
                        let mut tr = RECT { left: x + pad, top: ir.top, right: x + wd - pad, bottom: ir.bottom };
                        DrawTextW(hdc, t.as_ptr(), -1, &mut tr, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX);
                    }
                    x += wd;
                }
                SelectObject(hdc, old);
                CDRF_SKIPDEFAULT as isize
            }
            x if x == (CDDS_ITEMPREPAINT | CDDS_SUBITEM) => {
                let (bg, fg) = self.row_colors(cd.nmcd.dwItemSpec as isize);
                cd.clrTextBk = bg;
                cd.clrText = fg;
                cd.nmcd.uItemState &= !(CDIS_SELECTED | CDIS_FOCUS);
                CDRF_NEWFONT as isize
            }
            CDDS_POSTPAINT => {
                // thin lines between the columns, from under the header to the last row
                let mut cr: RECT = std::mem::zeroed();
                GetClientRect(c.list, &mut cr);
                let mut hr: RECT = std::mem::zeroed();
                GetWindowRect(c.header, &mut hr);
                let head_h = hr.bottom - hr.top;
                let scroll = GetScrollPos(c.list, SB_HORZ);
                let line = CreateSolidBrush(p.line);
                // thin line under every row (drawn here, once, after all rows)
                let top = SendMessageW(c.list, LVM_GETTOPINDEX, 0, 0).max(0) as usize;
                let per_page = SendMessageW(c.list, LVM_GETCOUNTPERPAGE, 0, 0).max(0) as usize + 1;
                let total = self.view.borrow().len();
                let mut rows_bottom = head_h; // column lines stop at the last row; empty space below stays plain
                for row in top..(top + per_page).min(total) {
                    let mut ir: RECT = std::mem::zeroed();
                    ir.left = LVIR_BOUNDS as i32;
                    if SendMessageW(c.list, LVM_GETITEMRECT, row, &mut ir as *mut RECT as isize) != 0 && ir.bottom > head_h {
                        FillRect(cd.nmcd.hdc, &RECT { left: 0, top: ir.bottom - 1, right: cr.right, bottom: ir.bottom }, line);
                        rows_bottom = rows_bottom.max(ir.bottom);
                    }
                }
                // plain background below the last row, whatever was painted there before
                if rows_bottom < cr.bottom {
                    let bgb = CreateSolidBrush(p.field);
                    FillRect(cd.nmcd.hdc, &RECT { left: 0, top: rows_bottom, right: cr.right, bottom: cr.bottom }, bgb);
                    DeleteObject(bgb as HGDIOBJ);
                }
                let mut x = -scroll;
                for i in 0..COLS.len() - 1 {
                    x += SendMessageW(c.list, LVM_GETCOLUMNWIDTH, i, 0) as i32;
                    if x - 1 >= 0 && x - 1 < cr.right {
                        let r = RECT { left: x - 1, top: head_h, right: x, bottom: rows_bottom.min(cr.bottom) };
                        FillRect(cd.nmcd.hdc, &r, line);
                    }
                }
                DeleteObject(line as HGDIOBJ);
                CDRF_DODEFAULT as isize
            }
            _ => CDRF_DODEFAULT as isize,
        }
    }

    // ------------------------------------------------------------------ messages
    unsafe fn on_notify(&self, nm: *const NMHDR) -> isize {
        let c = self.ctl.get();
        let h = &*nm;
        if h.hwndFrom == c.tip && h.code == TTN_GETDISPINFOW {
            let di = &mut *(nm as *mut NMTTDISPINFOW);
            let text = if self.dark.get() { "Switch to light mode" } else { "Switch to dark mode" };
            *self.tip_text.borrow_mut() = w(text);
            di.lpszText = self.tip_text.borrow().as_ptr() as *mut u16;
            return 0;
        }
        if h.hwndFrom == c.header {
            if h.code == HDN_ENDTRACKW || h.code == HDN_DIVIDERDBLCLICKW {
                PostMessageW(self.hwnd.get(), WM_TRACKED, 0, 0);
            }
            return 0;
        }
        if h.hwndFrom != c.list {
            return 0;
        }
        match h.code {
            NM_CUSTOMDRAW => self.custom_draw(nm as *mut NMLVCUSTOMDRAW),
            LVN_GETDISPINFOW => {
                let di = &mut *(nm as *mut NMLVDISPINFOW);
                if di.item.mask & LVIF_TEXT != 0 && !di.item.pszText.is_null() && di.item.cchTextMax > 0 {
                    let text: Vec<u16> = self.cell_text(di.item.iItem as usize, di.item.iSubItem as usize).encode_utf16().collect();
                    let n = text.len().min(di.item.cchTextMax as usize - 1);
                    std::ptr::copy_nonoverlapping(text.as_ptr(), di.item.pszText, n);
                    *di.item.pszText.add(n) = 0;
                }
                0
            }
            LVN_ITEMCHANGED => {
                let n = &*(nm as *const NMLISTVIEW);
                if n.uChanged & LVIF_STATE != 0 && (n.uNewState ^ n.uOldState) & LVIS_SELECTED != 0 {
                    self.on_select();
                }
                0
            }
            LVN_COLUMNCLICK => {
                let n = &*(nm as *const NMLISTVIEW);
                self.sort_by(n.iSubItem as usize);
                0
            }
            NM_DBLCLK => {
                let n = &*(nm as *const NMITEMACTIVATE);
                if n.iItem >= 0 {
                    // which column was double-clicked?
                    let mut hti: LVHITTESTINFO = std::mem::zeroed();
                    hti.pt = n.ptAction;
                    let row = SendMessageW(c.list, LVM_SUBITEMHITTEST, 0, &mut hti as *mut LVHITTESTINFO as isize);
                    if row >= 0 && hti.iSubItem == 0 {
                        self.hide_pencil();
                        self.rename_row(row); // Project column: rename, same as the pencil
                    } else {
                        self.open_release();
                    }
                }
                0
            }
            NM_RCLICK => {
                let n = &*(nm as *const NMITEMACTIVATE);
                if n.iItem >= 0 {
                    let st = SendMessageW(c.list, LVM_GETITEMSTATE, n.iItem as usize, LVIS_SELECTED as isize) as u32;
                    if st & LVIS_SELECTED == 0 {
                        let mut off: LVITEMW = std::mem::zeroed();
                        off.stateMask = LVIS_SELECTED;
                        SendMessageW(c.list, LVM_SETITEMSTATE, usize::MAX, &off as *const LVITEMW as isize);
                        off.state = LVIS_SELECTED | LVIS_FOCUSED;
                        off.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
                        SendMessageW(c.list, LVM_SETITEMSTATE, n.iItem as usize, &off as *const LVITEMW as isize);
                    }
                    let mut pt: POINT = std::mem::zeroed();
                    GetCursorPos(&mut pt);
                    self.popup_menu(pt.x, pt.y);
                }
                0
            }
            LVN_KEYDOWN => {
                let k = &*(nm as *const NMLVKEYDOWN);
                if k.wVKey == VK_DELETE {
                    self.remove_selected();
                } else if k.wVKey == VK_F2 {
                    self.rename_selected();
                } else if k.wVKey == b'A' as u16 && GetKeyState(VK_CONTROL as i32) < 0 {
                    // Ctrl+A: select every listed project (a filter limits it); -1 = all items
                    let mut st: LVITEMW = std::mem::zeroed();
                    st.stateMask = LVIS_SELECTED;
                    st.state = LVIS_SELECTED;
                    SendMessageW(c.list, LVM_SETITEMSTATE, usize::MAX, &st as *const LVITEMW as isize);
                } else if k.wVKey == 0x41 /* A */ && GetKeyState(VK_CONTROL as i32) < 0 {
                    // Ctrl+A: select every listed row (the list view does not do this by itself)
                    let mut st: LVITEMW = std::mem::zeroed();
                    st.stateMask = LVIS_SELECTED;
                    st.state = LVIS_SELECTED;
                    SendMessageW(c.list, LVM_SETITEMSTATE, usize::MAX, &st as *const LVITEMW as isize);
                }
                0
            }
            LVN_BEGINLABELEDIT => {
                let di = &*(nm as *const NMLVDISPINFOW);
                let edit = SendMessageW(c.list, LVM_GETEDITCONTROL, 0, 0) as HWND;
                if !edit.is_null() {
                    SendMessageW(edit, EM_SETLIMIT, MAX_CUSTOM_NAME * 2, 0);
                    SendMessageW(edit, EM_SETSEL, 0, -1);
                }
                let _ = di;
                0
            }
            LVN_ENDLABELEDIT => {
                let di = &*(nm as *const NMLVDISPINFOW);
                if !di.item.pszText.is_null() {
                    let text = from_wide_ptr(di.item.pszText);
                    let key = self.view.borrow().get(di.item.iItem as usize).map(|&i| project_key(&self.data.borrow().projects[i]));
                    if let Some(k) = key {
                        *self.pending_rename.borrow_mut() = Some((k, text));
                        PostMessageW(self.hwnd.get(), WM_RENAMED, 0, 0); // applied after the edit box is gone
                    }
                }
                0
            }
            _ => 0,
        }
    }

    unsafe fn on_command(&self, id: i32, notify: u32, from: HWND) {
        let _ = from;
        match id {
            ID_ADD => self.add_project(),
            ID_CHECK => self.check_updates(),
            ID_SEEN => self.mark_seen(),
            ID_OPEN => self.open_release(),
            ID_REMOVE => self.remove_selected(),
            ID_SETTINGS => dialogs::show_settings(self),
            ID_THEME => self.toggle_theme(),
            ID_PENCIL => self.pencil_clicked(),
            ID_ENTRY => {
                if notify == EN_SETFOCUS as u32 || notify == EN_KILLFOCUS as u32 {
                    let fr = self.entry_frame.get();
                    InvalidateRect(self.hwnd.get(), &fr, 1);
                }
            }
            ID_FILTER => {
                if notify == EN_CHANGE as u32 {
                    SetTimer(self.hwnd.get(), TIMER_FILTER, 150, None); // re-list shortly after the last keystroke
                } else if notify == EN_SETFOCUS as u32 || notify == EN_KILLFOCUS as u32 {
                    let fr = self.filter_frame.get();
                    InvalidateRect(self.hwnd.get(), &fr, 1);
                }
            }
            ID_FKIND => self.kind_menu(),
            ID_FCLEAR => self.clear_filter(),
            2 /* IDCANCEL: Esc */ => {
                if GetFocus() == self.ctl.get().filter {
                    self.clear_filter();
                }
            }
            ID_EXPORT => self.export_json(),
            ID_IMPORT => self.import_json(),
            1 /* IDOK: Enter */ => {
                if GetFocus() == self.ctl.get().entry {
                    self.add_project();
                }
            }
            _ => {
                let _ = notify;
            }
        }
    }

    fn apply_pending_rename(&self) {
        let pending = self.pending_rename.borrow_mut().take();
        if let Some((key, text)) = pending {
            let msg = {
                let mut d = self.data.borrow_mut();
                d.projects.iter_mut().find(|p| project_key(p) == key).and_then(|p| apply_rename(p, &text))
            };
            if let Some(m) = msg {
                self.persist();
                self.render();
                self.set_status(&m);
            }
        }
    }

    unsafe fn on_dpi_changed(&self, new_dpi: u32, rect: *const RECT) {
        self.dpi.set(new_dpi);
        let r = &*rect;
        SetWindowPos(self.hwnd.get(), null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
        self.make_fonts();
        self.apply_fonts();
        self.layout();
    }
}

fn post_job(target: usize, job: Job) {
    let raw = Box::into_raw(Box::new(job));
    unsafe {
        if PostMessageW(target as HWND, WM_JOB, 0, raw as isize) == 0 {
            drop(Box::from_raw(raw));
        }
    }
}

const BUTTONS: [(i32, u16, &str); 11] = [
    (ID_FKIND, G_FILTER, ""),
    (ID_FCLEAR, G_CLEAR, "Clear"),
    (ID_ADD, G_PLUS, "Add project"),
    (ID_CHECK, G_REFRESH, "Check for updates"),
    (ID_SEEN, G_EYE, "Mark as seen"),
    (ID_OPEN, G_OPEN, "Open release page"),
    (ID_REMOVE, G_TRASH, "Remove"),
    (ID_SETTINGS, G_GEAR, "Settings"),
    (ID_THEME, G_MOON, ""),
    (ID_EXPORT, G_EXPORT, "Export"),
    (ID_IMPORT, G_IMPORT, "Import"),
];

fn json_filter() -> Vec<u16> {
    "JSON files (*.json)\0*.json\0All files (*.*)\0*.*\0\0".encode_utf16().collect()
}

fn open_file_dialog(owner: HWND) -> Option<String> {
    unsafe {
        let mut buf = vec![0u16; 1024];
        let filter = json_filter();
        let title = w("Import projects (.json)");
        let mut ofn: OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = owner;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = buf.as_mut_ptr();
        ofn.nMaxFile = buf.len() as u32;
        ofn.lpstrTitle = title.as_ptr();
        ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY;
        if GetOpenFileNameW(&mut ofn) != 0 { Some(from_wide_buf(&buf)) } else { None }
    }
}

fn save_file_dialog(owner: HWND, initial: &str) -> Option<String> {
    unsafe {
        let mut buf = vec![0u16; 1024];
        for (i, c) in initial.encode_utf16().take(1000).enumerate() {
            buf[i] = c;
        }
        let filter = json_filter();
        let title = w("Export projects (.json)");
        let ext = w("json");
        let mut ofn: OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = owner;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = buf.as_mut_ptr();
        ofn.nMaxFile = buf.len() as u32;
        ofn.lpstrTitle = title.as_ptr();
        ofn.lpstrDefExt = ext.as_ptr();
        ofn.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY;
        if GetSaveFileNameW(&mut ofn) != 0 { Some(from_wide_buf(&buf)) } else { None }
    }
}

#[allow(dead_code)]
fn file_exists(p: &str) -> bool {
    unsafe { GetFileAttributesW(w(p).as_ptr()) != u32::MAX }
}

pub unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let ap = APP.load(Ordering::SeqCst);
    if ap.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let app = &*ap;
    if app.hwnd.get().is_null() && msg != WM_NCCREATE && msg != WM_CREATE && msg != WM_GETMINMAXINFO && msg != WM_NCCALCSIZE {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    match msg {
        WM_CREATE => {
            app.on_create(hwnd);
            0
        }
        WM_SIZE => {
            app.hide_pencil();
            if wp != SIZE_MINIMIZED as usize {
                app.layout();
            }
            0
        }
        WM_GETMINMAXINFO => {
            if !app.hwnd.get().is_null() {
                let mm = &mut *(lp as *mut MINMAXINFO);
                let (mw, mh) = app.min_track();
                mm.ptMinTrackSize.x = mw;
                mm.ptMinTrackSize.y = mh;
            }
            0
        }
        WM_ERASEBKGND => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            FillRect(wp as HDC, &rc, app.brushes.get().bg);
            let c = app.ctl.get();
            for (fr, edit) in [(app.entry_frame.get(), c.entry), (app.filter_frame.get(), c.filter)] {
                if fr.right > fr.left {
                    let p = app.pal();
                    let focused = GetFocus() == edit;
                    let fill = CreateSolidBrush(p.field);
                    let pen = CreatePen(PS_SOLID, 1, if focused { p.sel_bg } else { p.border });
                    let (ob, op) = (SelectObject(wp as HDC, fill as HGDIOBJ), SelectObject(wp as HDC, pen as HGDIOBJ));
                    let e = app.s(8);
                    RoundRect(wp as HDC, fr.left, fr.top, fr.right, fr.bottom, e, e);
                    SelectObject(wp as HDC, ob);
                    SelectObject(wp as HDC, op);
                    DeleteObject(fill as HGDIOBJ);
                    DeleteObject(pen as HGDIOBJ);
                }
            }
            1
        }
        WM_LBUTTONDOWN => {
            // a click in the frame but outside the edit still focuses the edit
            let (x, y) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
            let c = app.ctl.get();
            for (fr, edit) in [(app.entry_frame.get(), c.entry), (app.filter_frame.get(), c.filter)] {
                if x >= fr.left && x < fr.right && y >= fr.top && y < fr.bottom {
                    SetFocus(edit);
                }
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            let p = app.pal();
            SetTextColor(wp as HDC, if lp as HWND == app.ctl.get().fcount { p.sub_fg } else { p.fg });
            SetBkColor(wp as HDC, p.bg);
            app.brushes.get().bg as LRESULT
        }
        WM_CTLCOLOREDIT => {
            let p = app.pal();
            SetTextColor(wp as HDC, p.fg);
            SetBkColor(wp as HDC, p.field);
            app.brushes.get().field as LRESULT
        }
        WM_DRAWITEM => {
            app.draw_button(&*(lp as *const DRAWITEMSTRUCT));
            1
        }
        WM_NOTIFY => app.on_notify(lp as *const NMHDR),
        WM_COMMAND => {
            app.on_command(loword(wp) as i32, hiword(wp), lp as HWND);
            0
        }
        WM_CONTEXTMENU => {
            if wp as HWND == app.ctl.get().status {
                let (mut x, mut y) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
                if lp == -1 {
                    let mut pt: POINT = std::mem::zeroed();
                    GetCursorPos(&mut pt);
                    x = pt.x;
                    y = pt.y;
                }
                let menu = CreatePopupMenu();
                AppendMenuW(menu, MF_STRING, M_COPY as usize, w("Copy").as_ptr());
                let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, x, y, 0, hwnd, null());
                DestroyMenu(menu);
                app.run_menu_command(cmd as u32);
                return 0;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_SETCURSOR => {
            // hand cursor over the rename pencil, like a link
            let pc = app.pencil.get();
            if (!pc.is_null() && wp as HWND == pc) || wp as HWND == GetDlgItem(hwnd, ID_THEME) {
                SetCursor(LoadCursorW(null_mut(), IDC_HAND));
                return 1;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_DPICHANGED => {
            app.on_dpi_changed(hiword(wp), lp as *const RECT);
            0
        }
        WM_JOB => {
            let job = Box::from_raw(lp as *mut Job);
            app.on_job(*job);
            0
        }
        WM_RELAYOUT => {
            app.apply_columns();
            0
        }
        WM_TIMER if wp == TIMER_FILTER => {
            KillTimer(hwnd, TIMER_FILTER);
            app.render();
            0
        }
        WM_TIMER if wp == TIMER_PENCIL => {
            app.pencil_tick();
            0
        }
        WM_TRACKED => {
            app.after_track();
            0
        }
        WM_RENAMED => {
            app.apply_pending_rename();
            0
        }
        WM_STARTUP => {
            if app.data.borrow().settings.check_on_launch && !app.data.borrow().projects.is_empty() {
                app.check_updates();
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Subclass of the list view: tells the app where the mouse is (hover pencil) and hides the pencil
/// while the list scrolls.
unsafe extern "system" fn list_sub(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    let ap = APP.load(Ordering::SeqCst);
    if !ap.is_null() {
        let app = &*ap;
        match msg {
            WM_MOUSEMOVE => app.list_mouse_move((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32),
            WM_MOUSEWHEEL | WM_VSCROLL | WM_HSCROLL | WM_KEYDOWN | WM_LBUTTONDOWN => app.hide_pencil(),
            _ => {}
        }
    }
    windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, msg, wp, lp)
}

/// Subclass of the list's header: we paint it ourselves (dark / light colours, bold text, sort arrow).
unsafe extern "system" fn header_sub(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    let ap = APP.load(Ordering::SeqCst);
    if !ap.is_null() {
        let app = &*ap;
        match msg {
            WM_PAINT => {
                app.header_paint(hwnd);
                return 0;
            }
            WM_ERASEBKGND => return 1,
            WM_MOUSEMOVE => {
                let mut t = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
                TrackMouseEvent(&mut t);
                InvalidateRect(hwnd, null(), 0);
            }
            WM_MOUSELEAVE | WM_LBUTTONUP => {
                InvalidateRect(hwnd, null(), 0);
            }
            _ => {}
        }
    }
    windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, msg, wp, lp)
}
