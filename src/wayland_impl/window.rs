use super::{Waywin, WaywinState};
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
pub(crate) struct State {
    pub size: (i32, i32),
    pub scale: f64,
}
impl State {
    pub fn logical_size(&self) -> (f64, f64) {
        (
            (self.size.0 as f64 * self.scale).round() / self.scale,
            (self.size.1 as f64 * self.scale).round() / self.scale,
        )
    }
    pub fn physical_size(&self) -> (u32, u32) {
        (
            (self.size.0 as f64 * self.scale).round() as u32,
            (self.size.1 as f64 * self.scale).round() as u32,
        )
    }
}
#[derive(Clone, Copy, Default)]
struct PendingConfigure {
    pub size: Option<(i32, i32)>,
}

pub struct WindowInner {
    surface: WlSurface,
    xdg_surface: XdgSurface,
    toplevel: XdgToplevel,

    decoration: Option<ZxdgToplevelDecorationV1>,
    pub(crate) viewport_scaling: Option<(WpViewport, WpFractionalScaleV1)>,

    // qhandle: QueueHandle<WaywinState>,
    // frame: AtomicBool,
    redraw: AtomicBool,

    pub(crate) state: Mutex<State>,
    pub(crate) prev_state: Mutex<State>,
    configure: Mutex<PendingConfigure>,

    // for HasDisplayHandle
    connection: Connection,

    signal: calloop::LoopSignal,
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
        let ok = self
            .redraw
            .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok();
        self.signal.wakeup();
        ok
    }
}
impl Drop for WindowInner {
    fn drop(&mut self) {
        if let Some((viewport, scaling)) = &self.viewport_scaling {
            scaling.destroy();
            viewport.destroy();
        }
        if let Some(decoration) = &self.decoration {
            decoration.destroy();
        }
        self.toplevel.destroy();
        self.xdg_surface.destroy();
        self.surface.destroy();
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
                xdg_surface,
                toplevel,
                decoration,
                connection: waywin.connection.clone(),
                state: Mutex::new(state),
                prev_state: Mutex::new(state),
                configure: Mutex::new(configure),
                // qhandle: waywin.qhandle.clone(),
                // frame: AtomicBool::new(false),
                redraw: AtomicBool::new(true),
                viewport_scaling,
                signal: waywin.event_loop.get_signal(),
            }
        });
        drop(freeze);

        Ok(Self { inner })
    }
}
impl Window {
    pub fn get_physical_size(&self) -> (u32, u32) {
        self.inner.state.lock().unwrap().physical_size()
    }
    pub fn get_logical_size(&self) -> (f64, f64) {
        self.inner.state.lock().unwrap().logical_size()
    }
    pub fn get_scale(&self) -> f64 {
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
        _state: &mut Self,
        proxy: &WlSurface,
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
                proxy.set_buffer_scale(factor as i32);
                state.scale = factor;
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
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<XdgSurface, Weak<WindowInner>> for WaywinState {
    fn event(
        _state: &mut Self,
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

                let mut configure = data.configure.lock().unwrap();
                let mut state = data.state.lock().unwrap();
                match configure.size {
                    Some(configure_size) => {
                        state.size = configure_size;
                    }
                    None => configure.size = Some(state.size),
                }
                if let Some((viewport, _)) = &data.viewport_scaling {
                    viewport.set_destination(state.size.0, state.size.1);
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
                // state.hook(WindowEvent {
                //     kind: Event::Close,
                //     window_id: data.id(),
                // });
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
        _state: &mut Self,
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
                state.scale = scale;
            }
            _ => unimplemented!(),
        }
    }
}
