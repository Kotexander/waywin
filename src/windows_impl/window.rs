use super::{utils::*, EventHook, Waywin};
use crate::event::*;
use raw_window_handle as rwh;
use windows_sys::Win32::{Foundation::*, System::SystemServices::*, UI::WindowsAndMessaging::*};

const WM_WW_DESTROY: u32 = WM_USER + 1;

pub struct WindowData {
    /// should only be dereferenced in wndproc
    event_hook: *mut EventHook,
}
unsafe impl Send for WindowData {}
unsafe impl Sync for WindowData {}

pub struct Window {
    hwnd: usize,
    // Windows keeps a pointer to this so **do not move it**
    _info: Box<WindowData>,
}
impl Window {
    pub fn new(waywin: &Waywin, title: &str) -> std::result::Result<Self, String> {
        let info = Box::new(WindowData {
            event_hook: waywin.event_hook.as_ptr(),
        });

        let hwnd = create_window(
            waywin.window_class.as_ptr(),
            to_wide_str(title).as_ptr(),
            // std::ptr::null(),
            info.as_ref() as *const _ as *const _,
        )
        .map_err(|err| err.message())?;

        // if let Err(err) = set_dark_mode(hwnd.as_ptr(), true) {
        //     log::error!("{err}")
        // }

        Ok(Self {
            hwnd: hwnd.as_ptr() as usize,
            _info: info,
        })
    }
    pub fn hwnd(&self) -> HWND {
        self.hwnd as *mut _
    }
}
impl Window {
    pub fn get_size(&self) -> (u32, u32) {
        let rect = get_client_rect(self.hwnd()).unwrap();
        let (w, h) = get_size(rect);
        (w as u32, h as u32)
    }
    pub fn get_pos(&self) -> (i32, i32) {
        let rect = get_window_rect(self.hwnd()).unwrap();
        (rect.left, rect.top)
    }
    pub fn get_mouse_pos(&self) -> (i32, i32) {
        let point = screen_to_client(self.hwnd(), get_cursor_pos().unwrap()).unwrap();
        (point.x, point.y)
    }
    pub fn get_scale_factor(&self) -> f64 {
        to_scale_factor(get_dpi(self.hwnd()))
    }
}
impl Window {
    pub fn show(&self) -> bool {
        show_window(self.hwnd(), SW_SHOWNORMAL)
    }
    pub fn hide(&self) -> bool {
        show_window(self.hwnd(), SW_HIDE)
    }
    pub fn request_redraw(&self) {
        if let Err(err) = redraw_window(self.hwnd()) {
            log::error!("{err}");
        }
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        // Send a custom destroy message so the window can be destroyed on the correct thread.
        let _ = send_message(self.hwnd(), WM_WW_DESTROY, 0, 0);
    }
}

impl rwh::HasWindowHandle for Window {
    fn window_handle(&self) -> std::result::Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let mut window_handle = rwh::Win32WindowHandle::new(unsafe {
            // SAFETY: hwnd is already non null
            std::num::NonZeroIsize::new_unchecked(self.hwnd as isize)
        });
        let hinstance = unsafe { GetWindowLongW(self.hwnd(), GWLP_HINSTANCE) };
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
            let create = lparam as *const CREATESTRUCTW;
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
            let w = loword(lparam as usize) as u32;
            let h = hiword(lparam as usize) as u32;
            (Some(Event::Resize(w, h)), true)
        }
        WM_PAINT => {
            validate_rect(window);
            (Some(Event::Paint), true)
        }
        WM_DPICHANGED => {
            let rect = unsafe { &*(lparam as *const RECT) };
            let (w, h) = get_size(*rect);
            let x = rect.left;
            let y = rect.top;
            set_window_pos(window, x, y, w, h).unwrap();

            // x dpi and y dpi should be identical for windows apps
            // loword is chosen because it uses less instructions
            (
                Some(Event::NewScaleFactor(
                    to_scale_factor(loword(wparam) as u32),
                )),
                true,
            )
        }
        WM_MOUSEMOVE => {
            let x = loword(lparam as usize) as i16 as i32;
            let y = hiword(lparam as usize) as i16 as i32;

            let mods = wparam as u32;

            let modifier = MouseModifier {
                ctrl: mods & MK_CONTROL != 0,
                lbtn: mods & MK_LBUTTON != 0,
                rbtn: mods & MK_RBUTTON != 0,
                shift: mods & MK_SHIFT != 0,
                x1btn: mods & MK_XBUTTON1 != 0,
                x2btn: mods & MK_XBUTTON1 != 0,
            };

            (Some(Event::MouseMoved((x, y), modifier)), true)
        }
        WM_WW_DESTROY => {
            // show_window(window, SW_HIDE);
            if let Err(err) = destroy_window(window) {
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
                window_id: window as usize,
            });
        }
    }

    if handled {
        0
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
