use crate::event::{Event, WindowEvent};

use super::Waywin;
use raw_window_handle as rwh;
use std::{
    ptr::NonNull,
    sync::{Arc, Mutex},
};
use wayland_client::{
    protocol::{
        wl_callback::{self, WlCallback},
        wl_surface::{self, WlSurface},
    },
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
};

struct State {
    size: (u32, u32),
    surface: WlSurface,
}

pub struct Window {
    surface: WlSurface,
    _xdg_surface: XdgSurface,
    _toplevel: XdgToplevel,

    connection: Arc<Connection>,
    state: Arc<Mutex<State>>,
}
impl Window {
    pub fn new(waywin: &Waywin, title: &str) -> Result<Self, String> {
        let surface = {
            waywin
                .state
                .compositor
                .as_ref()
                .unwrap()
                .create_surface(&waywin.qhandle, ())
        };
        let xdg_surface = waywin.state.xdg_wm_base.as_ref().unwrap().get_xdg_surface(
            &surface,
            &waywin.qhandle,
            (),
        );
        let state = Arc::new(Mutex::new(State {
            size: (800, 600),
            surface: surface.clone(),
        }));

        let toplevel = xdg_surface.get_toplevel(&waywin.qhandle, state.clone());
        toplevel.set_title(title.to_owned());

        surface.frame(&waywin.qhandle, state.clone());
        surface.commit();

        Ok(Self {
            surface,
            connection: waywin.connection.clone(),
            state,
            _xdg_surface: xdg_surface,
            _toplevel: toplevel,
        })
    }
}
impl Window {
    pub fn get_size(&self) -> (u32, u32) {
        self.state.lock().unwrap().size
    }
    pub fn get_pos(&self) -> (i32, i32) {
        (0, 0)
    }

    pub fn get_scale_factor(&self) -> f64 {
        1.0
    }
}
impl Window {
    pub fn request_redraw(&self) {}
}

impl rwh::HasWindowHandle for Window {
    fn window_handle(&self) -> std::result::Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let ptr = self.surface.id().as_ptr();
        let handle = rwh::WaylandWindowHandle::new(NonNull::new(ptr as *mut _).unwrap());
        unsafe { Ok(rwh::WindowHandle::borrow_raw(handle.into())) }
    }
}
impl rwh::HasDisplayHandle for Window {
    fn display_handle(&self) -> std::result::Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let ptr = self.connection.display().id().as_ptr();
        let handle = rwh::WaylandDisplayHandle::new(NonNull::new(ptr as *mut _).unwrap());
        unsafe { Ok(rwh::DisplayHandle::borrow_raw(handle.into())) }
    }
}

impl Dispatch<WlSurface, ()> for super::State {
    fn event(
        _state: &mut Self,
        _proxy: &WlSurface,
        event: <WlSurface as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::info!("WlSurface {event:?}");

        match event {
            wl_surface::Event::Enter { output: _ } => {}
            wl_surface::Event::Leave { output: _ } => todo!(),
            wl_surface::Event::PreferredBufferScale { factor: _ } => {
                // todo!()
            }
            wl_surface::Event::PreferredBufferTransform { transform: _ } => {
                log::warn!("Preffered buffer transform not implemented.")
            }
            _ => todo!(),
        }
    }
}
impl Dispatch<XdgSurface, ()> for super::State {
    fn event(
        _state: &mut Self,
        proxy: &XdgSurface,
        event: <XdgSurface as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::info!("XdgSurface {event:?}");

        match event {
            xdg_surface::Event::Configure { serial } => {
                proxy.ack_configure(serial);
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<XdgToplevel, Arc<Mutex<State>>> for super::State {
    fn event(
        state: &mut Self,
        _proxy: &XdgToplevel,
        event: <XdgToplevel as wayland_client::Proxy>::Event,
        data: &Arc<Mutex<State>>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::info!("XdgToplevel {event:?}");

        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states: _,
            } => {
                let width = width as u32;
                let height = height as u32;
                if !(width == 0 && height == 0) {
                    let mut d = data.lock().unwrap();
                    if d.size.0 != width || d.size.1 != height {
                        d.size = (width, height);
                        drop(d);
                        state.hook(WindowEvent {
                            kind: Event::Resize(width, height),
                            window_id: 0,
                        });
                    }
                }
            }
            xdg_toplevel::Event::Close => {
                state.hook(WindowEvent {
                    kind: Event::Close,
                    window_id: 0,
                });
            }
            xdg_toplevel::Event::ConfigureBounds {
                width: _,
                height: _,
            } => todo!(),
            xdg_toplevel::Event::WmCapabilities { capabilities: _ } => {}
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<WlCallback, Arc<Mutex<State>>> for super::State {
    fn event(
        state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as wayland_client::Proxy>::Event,
        data: &Arc<Mutex<State>>,
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        // log::info!("WlCallback {event:?}");

        match event {
            wl_callback::Event::Done { callback_data: _ } => {
                state.hook(WindowEvent {
                    kind: Event::Paint,
                    window_id: 0,
                });

                let d = data.lock().unwrap();
                d.surface.damage(0, 0, d.size.0 as i32, d.size.1 as i32);
                d.surface.frame(qhandle, data.clone());
                d.surface.commit();
            }
            _ => unimplemented!(),
        }
    }
}
