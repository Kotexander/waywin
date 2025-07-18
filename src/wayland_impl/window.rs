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
use wayland_protocols::{
    wp::{
        fractional_scale::v1::client::wp_fractional_scale_v1::{self, WpFractionalScaleV1},
        viewporter::client::wp_viewport::WpViewport,
    },
    xdg::{
        decoration::zv1::client::zxdg_toplevel_decoration_v1::{Mode, ZxdgToplevelDecorationV1},
        shell::client::{
            xdg_surface::{self, XdgSurface},
            xdg_toplevel::{self, XdgToplevel},
        },
    },
};

#[derive(Clone, Copy)]
struct State {
    size: (i32, i32),
    scale: f64,
}
impl State {
    fn scaled_size(&self) -> (f64, f64) {
        (
            (self.size.0 as f64 * self.scale),
            (self.size.1 as f64 * self.scale),
        )
    }
    fn physical_size(&self) -> (u32, u32) {
        let size = self.scaled_size();
        (size.0.round() as u32, size.1.round() as u32)
    }
}
#[derive(Clone, Copy, Default)]
struct PendingConfigure {
    size: Option<(i32, i32)>,
}

pub struct WindowInner {
    surface: WlSurface,
    _xdg_surface: XdgSurface,
    toplevel: XdgToplevel,

    _decoration: Option<ZxdgToplevelDecorationV1>,
    viewport_scaling: Option<(WpViewport, WpFractionalScaleV1)>,

    // qhandle: QueueHandle<WaywinState>,
    // frame: AtomicBool,
    redraw: AtomicBool,

    // state and configure shouln't be modified on other threads
    state: Mutex<State>,
    configure: Mutex<PendingConfigure>,

    // for HasDisplayHandle
    connection: Connection,
}
impl WindowInner {
    pub fn id(&self) -> usize {
        self.surface.id().as_ptr() as usize
    }
    // pub fn frame(self: &Arc<Self>) {
    //     if self
    //         .frame
    //         .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
    //         .is_ok()
    //     {
    //         self.surface.frame(&self.qhandle, Arc::downgrade(self));
    //     }
    // }
    pub fn set_redraw(&self) {
        self.redraw.store(true, Ordering::Relaxed);
        // self.inner.frame();
    }
    pub fn reset_redraw(&self) -> bool {
        self.redraw
            .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }
}

pub struct Window {
    inner: Arc<WindowInner>,
}
impl Window {
    pub fn new(waywin: &mut Waywin, title: &str) -> Result<Self, String> {
        // this freeze might not be needed since a window shouldn't be created while the event queue is polled
        let freeze = waywin.qhandle.freeze();
        let inner = Arc::new_cyclic(|weak| {
            let surface = {
                waywin
                    .compositor
                    .create_surface(&waywin.qhandle, weak.clone())
            };
            let xdg_surface =
                waywin
                    .xdg_wm_base
                    .get_xdg_surface(&surface, &waywin.qhandle, weak.clone());
            let toplevel = xdg_surface.get_toplevel(&waywin.qhandle, weak.clone());
            toplevel.set_title(title.to_owned());
            toplevel.set_app_id(waywin.app_id.clone());

            let decoration = waywin.decoration.as_ref().map(|decoration| {
                let decor =
                    decoration.get_toplevel_decoration(&toplevel, &waywin.qhandle, weak.clone());
                decor.set_mode(Mode::ServerSide);
                decor
            });

            let viewport_scaling = waywin.viewporter.as_ref().zip(waywin.scaling.as_ref()).map(
                |(viewporter, scaling)| {
                    (
                        viewporter.get_viewport(&surface, &waywin.qhandle, weak.clone()),
                        scaling.get_fractional_scale(&surface, &waywin.qhandle, weak.clone()),
                    )
                },
            );

            surface.commit();

            let state = State {
                size: (800, 600),
                scale: 1.0,
            };
            let configure = PendingConfigure::default();

            waywin.windows.push(weak.clone());

            WindowInner {
                surface,
                _xdg_surface: xdg_surface,
                toplevel,
                _decoration: decoration,
                connection: waywin.connection.clone(),
                state: Mutex::new(state),
                configure: Mutex::new(configure),
                // qhandle: waywin.qhandle.clone(),
                // frame: AtomicBool::new(false),
                redraw: AtomicBool::new(false),
                viewport_scaling,
            }
        });
        drop(freeze);

        Ok(Self { inner })
    }
}
impl Window {
    pub fn get_size(&self) -> (u32, u32) {
        let size = self.inner.state.lock().unwrap().scaled_size();
        (size.0.round() as u32, size.1.round() as u32)
    }
    pub fn get_scale_factor(&self) -> f64 {
        self.inner.state.lock().unwrap().scale
    }
    pub fn set_title(&self, title: &str) {
        self.inner.toplevel.set_title(title.to_owned());
    }
    pub fn request_redraw(&self) {
        self.inner.set_redraw();
        // self.inner.frame();
    }
    pub fn id(&self) -> usize {
        self.inner.id()
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
        let Some(data) = data.upgrade() else {
            return;
        };

        match event {
            wl_surface::Event::Enter { output: _ } => {}
            wl_surface::Event::Leave { output: _ } => {}
            wl_surface::Event::PreferredBufferScale { factor } => {
                // if fractional scaling is supported
                // ignore this surface event
                if data.viewport_scaling.is_some() {
                    return;
                }

                // fallback if viewporter or fractional scaling is not supported
                let factor = factor as f64;
                let mut state = data.state.lock().unwrap();
                // let mut configure = data.configure.lock().unwrap();
                if state.scale != factor {
                    state.scale = factor;
                    // configure.scaled = true;
                    data.surface.set_buffer_scale(factor as i32);
                    let size = state.physical_size();

                    drop(state);
                    // drop(configure);

                    waywin_state.hook(WindowEvent {
                        kind: Event::NewScaleFactor(factor),
                        window_id: data.id(),
                    });
                    waywin_state.hook(WindowEvent {
                        kind: Event::Resize(size.0, size.1),
                        window_id: data.id(),
                    });
                    data.set_redraw();
                }
            }
            wl_surface::Event::PreferredBufferTransform { transform: _ } => {}
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<WlCallback, Weak<WindowInner>> for WaywinState {
    fn event(
        _state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as wayland_client::Proxy>::Event,
        weak: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let Some(_data) = weak.upgrade() else {
            return;
        };
        match event {
            wl_callback::Event::Done { callback_data: _ } => {
                todo!()
                // data.frame.store(false, Ordering::SeqCst);

                // state.hook(WindowEvent {
                //     kind: Event::Paint,
                //     window_id: data.id(),
                // });
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
        match event {
            xdg_surface::Event::Configure { serial } => {
                proxy.ack_configure(serial);

                let Some(data) = data.upgrade() else {
                    return;
                };

                let mut state = data.state.lock().unwrap();
                let mut configure = data.configure.lock().unwrap();

                let mut resize = false;

                // if configure.scaled {
                //     resize = true;
                //     configure.scaled = false;
                // }

                if let Some(conf_size) = configure.size {
                    if conf_size != state.size {
                        resize = true;
                        state.size = conf_size;
                        if let Some((viewport, _)) = &data.viewport_scaling {
                            viewport.set_destination(state.size.0, state.size.1);
                        }
                    }
                } else {
                    configure.size = Some(state.size);
                    if let Some((viewport, _)) = &data.viewport_scaling {
                        viewport.set_destination(state.size.0, state.size.1);
                    }
                    data.set_redraw();
                }

                if resize {
                    let size = state.physical_size();

                    drop(state);
                    drop(configure);

                    waywin_state.hook(WindowEvent {
                        kind: Event::Resize(size.0, size.1),
                        window_id: data.id(),
                    });
                    data.set_redraw();
                }
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<XdgToplevel, Weak<WindowInner>> for WaywinState {
    fn event(
        state: &mut Self,
        _proxy: &XdgToplevel,
        event: <XdgToplevel as wayland_client::Proxy>::Event,
        data: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let Some(data) = data.upgrade() else {
            return;
        };
        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states: _,
            } => {
                if !(width == 0 && height == 0) {
                    data.configure.lock().unwrap().size = Some((width, height))
                }
            }
            xdg_toplevel::Event::Close => {
                state.hook(WindowEvent {
                    kind: Event::Close,
                    window_id: data.id(),
                });
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
        _event: <ZxdgToplevelDecorationV1 as Proxy>::Event,
        _data: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // TODO
    }
}
impl Dispatch<WpViewport, Weak<WindowInner>> for WaywinState {
    fn event(
        _state: &mut Self,
        _proxy: &WpViewport,
        _event: <WpViewport as Proxy>::Event,
        _data: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WpFractionalScaleV1, Weak<WindowInner>> for WaywinState {
    fn event(
        waywin_state: &mut Self,
        _proxy: &WpFractionalScaleV1,
        event: <WpFractionalScaleV1 as Proxy>::Event,
        data: &Weak<WindowInner>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let Some(data) = data.upgrade() else {
            return;
        };

        match event {
            wp_fractional_scale_v1::Event::PreferredScale { scale } => {
                let scale = scale as f64 / 120.0;

                let mut state = data.state.lock().unwrap();
                // let mut configure = data.configure.lock().unwrap();

                if state.scale != scale {
                    state.scale = scale;
                    // configure.scaled = true;
                    let size = state.physical_size();

                    drop(state);
                    // drop(configure);

                    waywin_state.hook(WindowEvent {
                        kind: Event::NewScaleFactor(scale),
                        window_id: data.id(),
                    });
                    waywin_state.hook(WindowEvent {
                        kind: Event::Resize(size.0, size.1),
                        window_id: data.id(),
                    });
                    data.set_redraw();
                }
            }
            _ => unimplemented!(),
        }
    }
}
