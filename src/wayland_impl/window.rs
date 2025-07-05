use super::{Waywin, WaywinState};
use crate::event::{Event, WindowEvent};
use raw_window_handle as rwh;
use std::{
    ptr::NonNull,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
};
use wayland_client::{
    protocol::{
        wl_callback::{self, WlCallback},
        wl_surface::{self, WlSurface},
    },
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::xdg::{
    decoration::zv1::client::zxdg_toplevel_decoration_v1::{Mode, ZxdgToplevelDecorationV1},
    shell::client::{
        xdg_surface::{self, XdgSurface},
        xdg_toplevel::{self, XdgToplevel},
    },
};

#[derive(Clone, Copy)]
struct State {
    size: (u32, u32),
    scale: u32,
}
impl State {
    fn scaled_size(&self) -> (u32, u32) {
        (self.size.0 * self.scale, self.size.1 * self.scale)
    }
}
#[derive(Clone, Copy)]
struct PendingConfigure {
    size: (u32, u32),
}
struct WindowInner {
    surface: WlSurface,
    _xdg_surface: XdgSurface,
    _toplevel: XdgToplevel,
    _decoration: ZxdgToplevelDecorationV1,
    state: Mutex<State>,
    configure: Mutex<PendingConfigure>,
    qhandle: QueueHandle<WaywinState>,

    frame: AtomicBool,
    // for HasDisplayHandle
    connection: Connection,
}
impl WindowInner {
    fn id(&self) -> usize {
        self.surface.id().as_ptr() as usize
    }
    fn frame(self: &Arc<Self>) {
        if self
            .frame
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            self.surface.frame(&self.qhandle, Arc::downgrade(&self));
        }
    }
}

pub struct Window {
    inner: Arc<WindowInner>,
}
impl Window {
    pub fn new(waywin: &Waywin, title: &str) -> Result<Self, String> {
        // this freeze might not be needed since a window shouldn't be created while the event queue is
        // polled since windows must be created on the event queue's thread for win32
        let freeze = waywin.qhandle.freeze();
        let inner = Arc::new_cyclic(|weak| {
            let surface = {
                waywin
                    .state
                    .compositor
                    .as_ref()
                    .unwrap()
                    .create_surface(&waywin.qhandle, weak.clone())
            };
            let xdg_surface = waywin.state.xdg_wm_base.as_ref().unwrap().get_xdg_surface(
                &surface,
                &waywin.qhandle,
                weak.clone(),
            );

            let toplevel = xdg_surface.get_toplevel(&waywin.qhandle, weak.clone());
            toplevel.set_title(title.to_owned());

            let decoration = waywin
                .state
                .decoration
                .as_ref()
                .unwrap()
                .get_toplevel_decoration(&toplevel, &waywin.qhandle, weak.clone());
            decoration.set_mode(Mode::ServerSide);

            surface.frame(&waywin.qhandle, weak.clone());
            let frame = AtomicBool::new(true);
            surface.commit();

            let state = State {
                size: (800, 600),
                scale: 1,
            };
            let configure = PendingConfigure { size: state.size };

            WindowInner {
                surface,
                _xdg_surface: xdg_surface,
                _toplevel: toplevel,
                _decoration: decoration,
                connection: waywin.connection.clone(),
                state: Mutex::new(state),
                configure: Mutex::new(configure),
                frame,
                qhandle: waywin.qhandle.clone(),
            }
        });
        drop(freeze);

        Ok(Self { inner })
    }
}
impl Window {
    pub fn get_size(&self) -> (u32, u32) {
        self.inner.state.lock().unwrap().scaled_size()
    }

    pub fn get_scale_factor(&self) -> f64 {
        self.inner.state.lock().unwrap().scale as f64
    }
}
impl Window {
    pub fn request_redraw(&self) {
        self.inner.frame();
    }
}

impl rwh::HasWindowHandle for Window {
    fn window_handle(&self) -> std::result::Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let ptr = self.inner.surface.id().as_ptr();
        let handle = rwh::WaylandWindowHandle::new(NonNull::new(ptr as *mut _).unwrap());
        unsafe { Ok(rwh::WindowHandle::borrow_raw(handle.into())) }
    }
}
impl rwh::HasDisplayHandle for Window {
    fn display_handle(&self) -> std::result::Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let ptr = self.inner.connection.display().id().as_ptr();
        let handle = rwh::WaylandDisplayHandle::new(NonNull::new(ptr as *mut _).unwrap());
        unsafe { Ok(rwh::DisplayHandle::borrow_raw(handle.into())) }
    }
}

impl Dispatch<WlSurface, Weak<WindowInner>> for WaywinState {
    fn event(
        waywin_state: &mut Self,
        _proxy: &WlSurface,
        event: <WlSurface as wayland_client::Proxy>::Event,
        data: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("WlSurface {event:?}");

        let Some(data) = data.upgrade() else {
            return;
        };

        match event {
            wl_surface::Event::Enter { output: _ } => {}
            wl_surface::Event::Leave { output: _ } => {}
            wl_surface::Event::PreferredBufferScale { factor } => {
                let mut state = data.state.lock().unwrap();
                if state.scale != factor as u32 {
                    data.surface.set_buffer_scale(factor);
                    state.scale = factor as u32;

                    let size = state.scaled_size();
                    drop(state);

                    waywin_state.hook(WindowEvent {
                        kind: Event::NewScaleFactor(factor as f64),
                        window_id: data.id(),
                    });
                    waywin_state.hook(WindowEvent {
                        kind: Event::Resize(size.0, size.1),
                        window_id: data.id(),
                    });
                    waywin_state.hook(WindowEvent {
                        kind: Event::Paint,
                        window_id: data.id(),
                    });
                    let size = data.state.lock().unwrap().scaled_size();
                    data.surface
                        .damage_buffer(0, 0, size.0 as i32, size.1 as i32);
                    data.surface.commit();
                }
            }
            wl_surface::Event::PreferredBufferTransform { transform: _ } => {}
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<WlCallback, Weak<WindowInner>> for WaywinState {
    fn event(
        state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as wayland_client::Proxy>::Event,
        weak: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("WlCallback {event:?}");

        let Some(data) = weak.upgrade() else {
            return;
        };
        match event {
            wl_callback::Event::Done { callback_data: _ } => {
                data.frame.store(false, Ordering::SeqCst);

                state.hook(WindowEvent {
                    kind: Event::Paint,
                    window_id: data.id(),
                });

                let size = data.state.lock().unwrap().scaled_size();
                data.surface
                    .damage_buffer(0, 0, size.0 as i32, size.1 as i32);
                data.surface.commit();
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<XdgSurface, Weak<WindowInner>> for WaywinState {
    fn event(
        waywin_state: &mut Self,
        proxy: &XdgSurface,
        event: <XdgSurface as wayland_client::Proxy>::Event,
        data: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("XdgSurface {event:?}");

        match event {
            xdg_surface::Event::Configure { serial } => {
                proxy.ack_configure(serial);

                let Some(data) = data.upgrade() else {
                    return;
                };

                let configure = data.configure.lock().unwrap();
                let mut state = data.state.lock().unwrap();

                if state.size != configure.size {
                    state.size = configure.size;
                    let size = state.scaled_size();
                    drop(configure);
                    drop(state);

                    waywin_state.hook(WindowEvent {
                        kind: Event::Resize(size.0, size.1),
                        window_id: data.id(),
                    });
                    waywin_state.hook(WindowEvent {
                        kind: Event::Paint,
                        window_id: data.id(),
                    });

                    let size = data.state.lock().unwrap().scaled_size();
                    data.surface
                        .damage_buffer(0, 0, size.0 as i32, size.1 as i32);
                    data.surface.commit();
                }
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<XdgToplevel, Weak<WindowInner>> for WaywinState {
    fn event(
        _state: &mut Self,
        _proxy: &XdgToplevel,
        event: <XdgToplevel as wayland_client::Proxy>::Event,
        data: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("XdgToplevel {event:?}");

        let Some(data) = data.upgrade() else {
            return;
        };
        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states: _,
            } => {
                if width == 0 && height == 0 {
                } else {
                    data.configure.lock().unwrap().size = (width as u32, height as u32)
                }
            }
            xdg_toplevel::Event::Close => {
                todo!()
            }
            xdg_toplevel::Event::ConfigureBounds {
                width: _,
                height: _,
            } => {
                // TODO
            }
            xdg_toplevel::Event::WmCapabilities { capabilities: _ } => {
                // TODO
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<ZxdgToplevelDecorationV1, Weak<WindowInner>> for WaywinState {
    fn event(
        _state: &mut Self,
        _proxy: &ZxdgToplevelDecorationV1,
        event: <ZxdgToplevelDecorationV1 as Proxy>::Event,
        _data: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("ZxdgToplevelDecorationV1 {event:?}");
    }
}
