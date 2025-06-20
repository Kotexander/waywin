use crate::event::WindowEvent;
use class::WindowClass;
use std::cell::Cell;
pub use window::Window;
use windows::Win32::UI::{
    HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
    WindowsAndMessaging::{DispatchMessageW, GetMessageW, PostQuitMessage, MSG},
};

mod class;
mod pwstring;
mod utils;
mod window;

pub type EventHook = Option<Box<dyn FnMut(WindowEvent)>>;

pub struct Waywin {
    /// All created windows keep a pointer to this so **do not move it**
    event_hook: Box<Cell<EventHook>>,
    window_class: WindowClass,
}
impl Waywin {
    pub fn init(class_name: &str) -> std::result::Result<Self, String> {
        if let Err(err) =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
        {
            log::error!("failed to set dpi awarness: {err}");
        }
        //

        let window_class = WindowClass::new(class_name)?;

        Ok(Self {
            event_hook: Box::new(None.into()),
            window_class,
        })
    }
    pub fn exit(&self) {
        unsafe { PostQuitMessage(0) }
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

            while GetMessageW(&mut message as *mut _, None, 0, 0).as_bool() {
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
