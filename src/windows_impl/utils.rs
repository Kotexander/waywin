use std::{
    ffi::c_void,
    num::NonZero,
    ptr::{null, null_mut, NonNull},
};
use windows_result::*;
use windows_sys::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
};

macro_rules! error_on_false {
    ($expr: expr, $ok: expr) => {
        if $expr == FALSE {
            Err(Error::from_win32())
        } else {
            Ok($ok)
        }
    };
    ($expr: expr) => {
        error_on_false!($expr, ())
    };
}

// macro_rules! error_on_not_ok {
//     ($expr: expr, $ok: expr) => {{
//         let hresult = $expr;
//         if $expr != S_OK {
//             Err(Error::from_hresult(windows_result::HRESULT(hresult)))
//         } else {
//             Ok($ok)
//         }
//     }};
//     ($expr: expr) => {
//         error_on_not_ok!($expr, ())
//     };
// }

#[allow(clippy::upper_case_acronyms)]
pub type WNDPROC = unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize;

pub fn loword(l: usize) -> usize {
    l & 0xffff
}
pub fn hiword(l: usize) -> usize {
    (l >> 16) & 0xffff
}
pub fn instance() -> HINSTANCE {
    // TODO:
    // https://devblogs.microsoft.com/oldnewthing/20041025-00/?p=37483
    // https://stackoverflow.com/questions/21718027/getmodulehandlenull-vs-hinstance
    unsafe { GetModuleHandleW(null()) }
}
pub fn to_wide_str(str: &str) -> Vec<u16> {
    str.encode_utf16().chain(std::iter::once(0)).collect()
}
// pub fn from_wide_str(str: &[u16]) -> String {
//     String::from_utf16_lossy(&str[..str.len() - 2])
// }
pub fn to_scale_factor(dpi: u32) -> f64 {
    dpi as f64 / USER_DEFAULT_SCREEN_DPI as f64
}
pub fn get_size(rect: RECT) -> (i32, i32) {
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    (w, h)
}

/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclientrect
pub fn get_client_rect(hwnd: HWND) -> Result<RECT> {
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        error_on_false!(GetClientRect(hwnd, std::ptr::addr_of_mut!(rect)), rect)
    }
}
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowrect
pub fn get_window_rect(hwnd: HWND) -> Result<RECT> {
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        error_on_false!(GetWindowRect(hwnd, std::ptr::addr_of_mut!(rect)), rect)
    }
}
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos
pub fn set_window_pos(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) -> Result<()> {
    unsafe {
        error_on_false!(SetWindowPos(
            hwnd,
            null_mut(),
            x,
            y,
            w,
            h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        ))
    }
}

/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setprocessdpiawarenesscontext
pub fn set_dpi_aware() -> Result<()> {
    unsafe {
        error_on_false!(SetProcessDpiAwarenessContext(
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
        ))
    }
}
///https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdpiforwindow
pub fn get_dpi(hwnd: HWND) -> u32 {
    unsafe { GetDpiForWindow(hwnd) }
}

// /// https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmsetwindowattribute
// pub fn set_dark_mode(hwnd: HWND, mode: bool) -> Result<()> {
//     unsafe {
//         let mode = mode as BOOL;
//         error_on_not_ok!(DwmSetWindowAttribute(
//             hwnd,
//             DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
//             std::ptr::addr_of!(mode) as *const _,
//             std::mem::size_of::<BOOL>() as u32
//         ))
//     }
// }

/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-redrawwindow
pub fn redraw_window(hwnd: HWND) -> Result<()> {
    unsafe { error_on_false!(RedrawWindow(hwnd, null(), null_mut(), RDW_INTERNALPAINT)) }
}
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-validaterect
pub fn validate_rect(hwnd: HWND) -> bool {
    unsafe { ValidateRect(hwnd, null()) == TRUE }
}

/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postquitmessage
pub fn post_quit_message(exit_code: i32) {
    unsafe { PostQuitMessage(exit_code) }
}
// /// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postmessagew
// pub fn post_message(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> Result<()> {
//     unsafe { error_on_false!(PostMessageW(hwnd, msg, wparam, lparam)) }
// }
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagew
pub fn send_message(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> LRESULT {
    unsafe { SendMessageW(hwnd, msg, wparam, lparam) }
}

/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerclassexw
pub fn register_class(class_name: PCWSTR, wndproc: Option<WNDPROC>) -> Result<NonZero<u16>> {
    let win_class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: wndproc,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance(),
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: (COLOR_WINDOW + 1) as _,
        lpszMenuName: null(),
        lpszClassName: class_name,
        hIconSm: null_mut(),
    };

    NonZero::new(unsafe { RegisterClassExW(&win_class) }).ok_or_else(Error::from_win32)
}
// /// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-unregisterclassw
// pub fn unregister_class(class_name: PCWSTR) -> Result<()> {
//     unsafe { error_on_false!(UnregisterClassW(class_name, instance())) }
// }

/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowexw
pub fn create_window(
    class_name: PCWSTR,
    title: PCWSTR,
    info: *const c_void,
) -> Result<NonNull<c_void>> {
    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_OVERLAPPEDWINDOW, // | WS_EX_ACCEPTFILES,
            class_name,
            title,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            null_mut(),
            null_mut(),
            instance(),
            info,
        );
        NonNull::new(hwnd).ok_or_else(Error::from_win32)
    }
}
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow
pub fn show_window(hwnd: HWND, cmd_show: i32) -> bool {
    unsafe { ShowWindow(hwnd, cmd_show) == TRUE }
}
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-destroywindow
pub fn destroy_window(hwnd: HWND) -> Result<()> {
    unsafe { error_on_false!(DestroyWindow(hwnd)) }
}
