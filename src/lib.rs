pub mod event;

use event::WindowEvent;
use raw_window_handle as rwh;
use std::{
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering},
};

#[cfg(target_os = "windows")]
mod windows_impl;
#[cfg(target_os = "windows")]
use windows_impl as backend_impl;

pub fn init(class_name: &str) -> Result<Waywin, String> {
    Waywin::init(class_name)
}


static WAYWIN_INIT: AtomicBool = AtomicBool::new(false);

/// Used to create windows and run the event runner.
pub struct Waywin {
    backend_impl: backend_impl::Waywin,
    _marker: PhantomData<*const ()>, // not `Send` or `Sync`
}
impl Waywin {
    pub fn init(class_name: &str) -> Result<Self, String> {
        if WAYWIN_INIT.swap(true, Ordering::Relaxed) {
            return Err("Waywin::init can only be called once".to_string());
        }

        backend_impl::Waywin::init(class_name).map(|backend_impl| Self {
            backend_impl,
            _marker: PhantomData,
        })
    }
    pub fn create_window(&self, title: &str) -> Result<Window, String> {
        backend_impl::Window::new(&self.backend_impl, title)
            .map(|backend_impl| Window { backend_impl })
    }
    pub fn exit(&self) {
        self.backend_impl.exit()
    }
    pub fn run(&self, event_hook: impl FnMut(WindowEvent)) {
        self.backend_impl.run(event_hook)
    }
}

pub struct Window {
    backend_impl: backend_impl::Window,
}
impl Window {
    pub fn get_size(&self) -> (u32, u32) {
        self.backend_impl.get_size()
    }
    pub fn get_pos(&self) -> (i32, i32) {
        self.backend_impl.get_pos()
    }
    pub fn get_mouse_pos(&self) -> (i32, i32) {
        self.backend_impl.get_mouse_pos()
    }
    pub fn get_scale_factor(&self) -> f64 {
        self.backend_impl.get_scale_factor()
    }
}
impl Window {
    pub fn show(&self) {
        self.backend_impl.show();
    }
    pub fn hide(&self) {
        self.backend_impl.hide();
    }
    pub fn request_redraw(&self) {
        self.backend_impl.request_redraw()
    }
}
impl rwh::HasWindowHandle for Window {
    fn window_handle(&self) -> std::result::Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        self.backend_impl.window_handle()
    }
}
impl rwh::HasDisplayHandle for Window {
    fn display_handle(&self) -> std::result::Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        self.backend_impl.display_handle()
    }
}
