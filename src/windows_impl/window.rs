use super::{
    pwstring::PWSTRING,
    utils::{hiword, instance, loword},
    EventHook, Waywin,
};
use crate::event::*;
use raw_window_handle as rwh;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{RedrawWindow, ValidateRect, RDW_INTERNALPAINT},
    System::SystemServices::{
        MK_CONTROL, MK_LBUTTON, MK_RBUTTON, MK_SHIFT, MK_XBUTTON1, MODIFIERKEYS_FLAGS,
    },
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowLongPtrW,
            GetWindowLongW, GetWindowRect, SendMessageW, SetWindowLongPtrW, SetWindowPos,
            CREATESTRUCTW, CW_USEDEFAULT, GWLP_HINSTANCE, GWLP_USERDATA, SWP_NOACTIVATE,
            SWP_NOZORDER, USER_DEFAULT_SCREEN_DPI, WM_CLOSE, WM_CREATE, WM_DPICHANGED,
            WM_MOUSEMOVE, WM_PAINT, WM_SIZE, WM_USER, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
        },
    },
};

const WM_WW_DESTROY: u32 = WM_USER + 1;

pub struct WindowData {
    /// should only be dereferenced in wndproc
    event_hook: *mut EventHook,
}
// unsafe impl Send for WindowData {}
// unsafe impl Sync for WindowData {}

pub struct Window {
    hwnd: HWND,
    /// Windows keeps a pointer to this so **do not move it**
    _info: Box<WindowData>,
}
unsafe impl Send for Window {}
unsafe impl Sync for Window {}
impl Window {
    pub fn new(waywin: &Waywin, title: &str) -> std::result::Result<Self, String> {
        let info = Box::new(WindowData {
            event_hook: waywin.event_hook.as_ptr(),
        });

        let hwnd = unsafe {
            CreateWindowExW(
                // WS_EX_APPWINDOW | WS_EX_OVERLAPPEDWINDOW,
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE::default(),
                waywin.window_class.name(),
                PWSTRING::from(title).as_pcwstr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                Some(instance()),
                Some(info.as_ref() as *const _ as *const _),
            )
        }
        .map_err(|err| format!("create window: {err}"))?;

        Ok(Self { hwnd, _info: info })
    }
}
impl Window {
    fn get_client_rect(&self) -> RECT {
        let mut rect: RECT = RECT::default();
        unsafe { GetClientRect(self.hwnd, std::ptr::addr_of_mut!(rect)).unwrap() }
        rect
    }
    fn get_window_rect(&self) -> RECT {
        let mut rect: RECT = RECT::default();
        unsafe { GetWindowRect(self.hwnd, std::ptr::addr_of_mut!(rect)).unwrap() }
        rect
    }

    pub fn get_size(&self) -> (u32, u32) {
        let (w, h) = get_size(self.get_client_rect());
        (w as u32, h as u32)
    }
    pub fn get_pos(&self) -> (i32, i32) {
        let rect = self.get_window_rect();
        (rect.left, rect.top)
    }

    pub fn get_scale_factor(&self) -> f64 {
        let dpi = unsafe { GetDpiForWindow(self.hwnd) };
        assert_ne!(dpi, 0);
        to_scale_factor(dpi)
    }
}
impl Window {
    pub fn request_redraw(&self) {
        if !unsafe { RedrawWindow(Some(self.hwnd), None, None, RDW_INTERNALPAINT) }.as_bool() {
            log::error!(
                "failed to request redraw for window: {}",
                self.hwnd.0 as usize
            );
        }
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        // Send a custom destroy message so the window can be destroyed on the correct thread.
        let _ = unsafe { SendMessageW(self.hwnd, WM_WW_DESTROY, None, None) };
    }
}

impl rwh::HasWindowHandle for Window {
    fn window_handle(&self) -> std::result::Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let mut window_handle =
            rwh::Win32WindowHandle::new(std::num::NonZeroIsize::new(self.hwnd.0 as isize).unwrap());
        let hinstance = unsafe { GetWindowLongW(self.hwnd, GWLP_HINSTANCE) };
        window_handle.hinstance = std::num::NonZeroIsize::new(hinstance as isize);

        Ok(unsafe { rwh::WindowHandle::borrow_raw(rwh::RawWindowHandle::Win32(window_handle)) })
    }
}
impl rwh::HasDisplayHandle for Window {
    fn display_handle(&self) -> std::result::Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let display_handle = rwh::RawDisplayHandle::Windows(rwh::WindowsDisplayHandle::new());
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(display_handle) })
    }
}

fn extract_info(hwnd: HWND, msg: u32, lparam: LPARAM) -> *const WindowData {
    unsafe {
        if msg == WM_CREATE {
            let create = lparam.0 as *const CREATESTRUCTW;
            let info = (*create).lpCreateParams as *const WindowData;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, info as isize);
            info
        } else {
            GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowData
        }
    }
}

fn translate_event(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> (Option<Event>, bool) {
    match message {
        WM_CLOSE => (Some(Event::Close), true),
        WM_SIZE => {
            let w = loword(lparam.0 as usize) as u32;
            let h = hiword(lparam.0 as usize) as u32;
            (Some(Event::Resize(w, h)), true)
        }
        WM_PAINT => {
            if !unsafe { ValidateRect(Some(window), None) }.as_bool() {
                log::error!("failed to validate rect for window: {}", window.0 as usize);
            }
            (Some(Event::Paint), true)
        }
        WM_DPICHANGED => {
            let rect = unsafe { &*(lparam.0 as *const RECT) };
            let (w, h) = get_size(*rect);
            let x = rect.left;
            let y = rect.top;

            if let Err(err) =
                unsafe { SetWindowPos(window, None, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE) }
            {
                log::error!("failed to set window position after dpi change: {err}");
            }

            // x dpi and y dpi should be identical for windows apps
            (
                Some(Event::NewScaleFactor(to_scale_factor(
                    loword(wparam.0) as u32
                ))),
                true,
            )
        }
        WM_MOUSEMOVE => {
            let x = loword(lparam.0 as usize) as i16 as i32;
            let y = hiword(lparam.0 as usize) as i16 as i32;

            let mods = MODIFIERKEYS_FLAGS(wparam.0 as u32);

            let modifier = MouseModifier {
                ctrl: mods.contains(MK_CONTROL),
                lbtn: mods.contains(MK_LBUTTON),
                rbtn: mods.contains(MK_RBUTTON),
                shift: mods.contains(MK_SHIFT),
                x1btn: mods.contains(MK_XBUTTON1),
                x2btn: mods.contains(MK_XBUTTON1),
            };

            (Some(Event::MouseMoved((x, y), modifier)), true)
        }
        WM_WW_DESTROY => {
            // show_window(window, SW_HIDE);
            if let Err(err) = unsafe { DestroyWindow(window) } {
                log::error!("error during destroy window: {err}");
            }
            (None, true)
        }
        _ => {
            // log::info!("msg: {message}");
            (None, false)
        }
    }
}

fn waywin_wndproc(
    data: &WindowData,
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let (event, handled) = translate_event(window, message, wparam, lparam);

    if let Some(event) = event {
        if let Some(hook) = unsafe { &mut *data.event_hook } {
            hook(WindowEvent {
                kind: event,
                window_id: window.0 as usize,
            });
        }
    }

    if handled {
        LRESULT(0)
    } else {
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }
}

pub extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let data = extract_info(window, message, lparam);

    let data = unsafe {
        if data.is_null() {
            return DefWindowProcW(window, message, wparam, lparam);
        }
        &*data
    };

    waywin_wndproc(data, window, message, wparam, lparam)
}

fn to_scale_factor(dpi: u32) -> f64 {
    dpi as f64 / USER_DEFAULT_SCREEN_DPI as f64
}
fn get_size(rect: RECT) -> (i32, i32) {
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    (w, h)
}
