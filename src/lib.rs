#[cfg(not(target_pointer_width = "64"))]
compile_error!("waywin only supports 64-bit targets.");

pub mod event;

use event::WindowEvent;
use raw_window_handle as rwh;
use std::marker::PhantomData;

#[cfg(target_os = "windows")]
mod windows_impl;
#[cfg(target_os = "windows")]
use windows_impl as backend_impl;

#[cfg(target_os = "linux")]
mod wayland_impl;
#[cfg(target_os = "linux")]
use wayland_impl as backend_impl;

/// Used to create windows and run the event runner.
pub struct Waywin {
    backend_impl: backend_impl::Waywin,
    _marker: PhantomData<*const ()>, // not `Send` or `Sync`
}
impl Waywin {
    pub fn init(class_name: &str) -> Result<Self, String> {
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
    pub fn run(&mut self, event_hook: impl FnMut(WindowEvent) + 'static) {
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
    pub fn get_scale_factor(&self) -> f64 {
        self.backend_impl.get_scale_factor()
    }
}
impl Window {
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
