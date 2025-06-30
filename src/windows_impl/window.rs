use super::{
    class::WindowClass,
    utils::{hiword, instance, loword, PWSTRING},
    EventHook, Waywin,
};
use crate::{
    event::*,
    windows_impl::utils::{get_x, get_y},
};
use raw_window_handle as rwh;
use std::rc::Rc;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{RedrawWindow, ValidateRect, RDW_INTERNALPAINT},
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowLongPtrW,
            GetWindowRect, PostMessageW, SetWindowLongPtrW, SetWindowPos, CREATESTRUCTW,
            CW_USEDEFAULT, GWLP_HINSTANCE, GWLP_USERDATA, SWP_NOACTIVATE, SWP_NOZORDER,
            USER_DEFAULT_SCREEN_DPI, WINDOW_EX_STYLE, WM_CLOSE, WM_CREATE, WM_DPICHANGED,
            WM_ERASEBKGND, WM_MOUSEMOVE, WM_NCCREATE, WM_PAINT, WM_SIZE, WM_USER, WS_CLIPCHILDREN,
            WS_CLIPSIBLINGS, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
        },
    },
};

const WAYWIN_DESTROY: u32 = WM_USER + 1;

pub struct CreateInfo {
    event_hook: EventHook,
    class: Rc<WindowClass>,
}
pub struct WindowData {
    event_hook: EventHook,
    window_id: usize,
    // make sure that the window class doesn't get
    // unregistered before this window is destroyed
    _class: Rc<WindowClass>,
}
impl WindowData {
    fn hook(&mut self, event: Event) {
        if let Some(hook) = unsafe { &mut *self.event_hook.get() } {
            hook(WindowEvent {
                kind: event,
                window_id: self.window_id,
            })
        }
    }
}

struct SyncHWND(HWND);
unsafe impl Send for SyncHWND {}
unsafe impl Sync for SyncHWND {}

pub struct Window {
    hwnd: SyncHWND,
}
impl Window {
    pub fn new(waywin: &Waywin, title: &str) -> Result<Self, String> {
        let info = CreateInfo {
            event_hook: waywin.event_hook.clone(),
            class: waywin.window_class.clone(),
        };

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                waywin.window_class.name(),
                PWSTRING::from(title).as_pcwstr(),
                WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                Some(instance()),
                Some(std::ptr::addr_of!(info) as _),
            )
        }
        .map_err(|err| format!("create window: {err}"))?;

        Ok(Self {
            hwnd: SyncHWND(hwnd),
        })
    }
}
impl Window {
    #[inline]
    fn hwnd(&self) -> HWND {
        self.hwnd.0
    }
    fn get_client_rect(&self) -> RECT {
        let mut rect: RECT = RECT::default();
        unsafe { GetClientRect(self.hwnd(), std::ptr::addr_of_mut!(rect)).unwrap() }
        rect
    }
    fn get_window_rect(&self) -> RECT {
        let mut rect: RECT = RECT::default();
        unsafe { GetWindowRect(self.hwnd(), std::ptr::addr_of_mut!(rect)).unwrap() }
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
        let dpi = unsafe { GetDpiForWindow(self.hwnd()) };
        assert_ne!(dpi, 0);
        to_scale_factor(dpi)
    }
}
impl Window {
    pub fn request_redraw(&self) {
        if !unsafe { RedrawWindow(Some(self.hwnd()), None, None, RDW_INTERNALPAINT) }.as_bool() {
            log::error!(
                "failed to request redraw for window: {}",
                self.hwnd().0 as usize
            );
        }
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        // Post a custom destroy message so the window can be destroyed on the correct thread.
        let _ = unsafe { PostMessageW(Some(self.hwnd()), WAYWIN_DESTROY, WPARAM(0), LPARAM(0)) };
    }
}

impl rwh::HasWindowHandle for Window {
    fn window_handle(&self) -> std::result::Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let mut window_handle = rwh::Win32WindowHandle::new(
            std::num::NonZeroIsize::new(self.hwnd().0 as isize).unwrap(),
        );
        let hinstance = unsafe { GetWindowLongPtrW(self.hwnd(), GWLP_HINSTANCE) };
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

pub extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let data = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut WindowData;
    let data = match (data.is_null(), message) {
        // called during CreateWindowEx
        (true, WM_NCCREATE) => {
            let create = lparam.0 as *const CREATESTRUCTW;
            let info = unsafe {
                ((*create).lpCreateParams as *const CreateInfo)
                    .as_ref()
                    .unwrap()
            };
            let data = Box::new(WindowData {
                event_hook: info.event_hook.clone(),
                window_id: window.0 as usize,
                _class: info.class.clone(),
            });
            unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, Box::into_raw(data) as isize) };
            return unsafe { DefWindowProcW(window, message, wparam, lparam) };
        }
        // ready to destroy and free memory
        (false, WAYWIN_DESTROY) => {
            drop(unsafe { Box::from_raw(data) });
            unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, 0) };
            if let Err(err) = unsafe { DestroyWindow(window) } {
                log::error!("error during destroy window: {err}");
            }
            return unsafe { DefWindowProcW(window, message, wparam, lparam) };
        }
        // sanity check data exists after NCCREATE, if not then return error
        (true, WM_CREATE) => return LRESULT(-1),
        // no data so don't handle anything
        (true, _) => return unsafe { DefWindowProcW(window, message, wparam, lparam) },
        // yes data and deref it
        (false, _) => unsafe { &mut (*data) },
    };

    match message {
        WM_CLOSE => {
            data.hook(Event::Close);
            LRESULT(0)
        }
        WM_SIZE => {
            let w = loword(lparam.0 as usize);
            let h = hiword(lparam.0 as usize);
            data.hook(Event::Resize(w, h));
            LRESULT(0)
        }
        WM_PAINT => {
            if !unsafe { ValidateRect(Some(window), None) }.as_bool() {
                log::error!("failed to validate rect for window: {}", window.0 as usize);
            }
            data.hook(Event::Paint);
            LRESULT(0)
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

            data.hook(Event::NewScaleFactor(to_scale_factor(
                loword(wparam.0) as u32
            )));
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let x = get_x(lparam.0 as usize) as i32;
            let y = get_y(lparam.0 as usize) as i32;

            // let mods = MODIFIERKEYS_FLAGS(wparam.0 as u32);

            // let modifier = MouseModifier {
            //     ctrl: mods.contains(MK_CONTROL),
            //     shift: mods.contains(MK_SHIFT),
            //     lbtn: mods.contains(MK_LBUTTON),
            //     rbtn: mods.contains(MK_RBUTTON),
            //     mbtn: mods.contains(MK_MBUTTON),
            //     x1btn: mods.contains(MK_XBUTTON1),
            //     x2btn: mods.contains(MK_XBUTTON2),
            // };
            //
            data.hook(Event::MouseMoved(x, y));
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn to_scale_factor(dpi: u32) -> f64 {
    dpi as f64 / USER_DEFAULT_SCREEN_DPI as f64
}
fn get_size(rect: RECT) -> (i32, i32) {
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    (w, h)
}
