mod utils;
mod window;

use crate::event::WindowEvent;
use std::{cell::Cell, ptr::null_mut};
use utils::*;
use windows_sys::Win32::{Graphics::Gdi::HBRUSH, UI::WindowsAndMessaging::*};

pub use window::Window;

pub type EventHook = Option<Box<dyn FnMut(WindowEvent)>>;

struct WindowClass {
    name: Vec<u16>,
    _background: HBRUSH,
}
impl WindowClass {
    fn new(name: &str) -> Result<Self, String> {
        let name = to_wide_str(name);
        let background = create_brush(0, 0, 0);
        register_class(name.as_ptr(), Some(window::wndproc), background)
            .map_err(|err| err.message())?;
        Ok(Self {
            name,
            _background: background,
        })
    }
    // fn as_string(&self) -> String {
    //     from_wide_str(&self.name)
    // }
    fn as_ptr(&self) -> *const u16 {
        self.name.as_ptr()
    }
}
// impl Drop for WindowClass {
//     fn drop(&mut self) {
//         if let Err(err) = unregister_class(self.name.as_ptr()) {
//             log::error!(
//                 "Failed to unregister class '{}': {}",
//                 from_wide_str(&self.name),
//                 err
//             );
//         }
//         if let Err(_) = delete_object(self.background) {
//             log::error!("Failed to deleted background brush");
//         }
//     }
// }

pub struct Waywin {
    /// All created windows keep a pointer to this so **do not move it**
    event_hook: Box<Cell<EventHook>>,
    window_class: WindowClass,
}
impl Waywin {
    pub fn init(class_name: &str) -> std::result::Result<Self, String> {
        set_dpi_aware().map_err(|err| err.message())?;
        let window_class = WindowClass::new(class_name)?;

        Ok(Self {
            event_hook: Box::new(None.into()),
            window_class,
        })
    }
    pub fn exit(&self) {
        post_quit_message(0);
    }
    pub fn run(&self, event_hook: impl FnMut(WindowEvent)) {
        let hook: Box<dyn FnMut(WindowEvent)> = unsafe {
            std::mem::transmute::<Box<dyn FnMut(WindowEvent)>, Box<dyn FnMut(WindowEvent)>>(
                Box::new(event_hook),
            )
        };
        self.event_hook.set(Some(hook));
        unsafe {
            let mut message: MSG = std::mem::zeroed();

            while GetMessageW(&mut message as *mut _, null_mut(), 0, 0) > 0 {
                DispatchMessageW(&message as *const _);
            }
        }
        self.event_hook.set(None);
    }
}
impl std::fmt::Debug for Waywin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waywin")
            // .field("event_hook", &self.event_hook)
            // .field("class_name", &self.class_name)
            .finish_non_exhaustive()
    }
}
