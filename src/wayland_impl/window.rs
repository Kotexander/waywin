use super::{state::pointer::PointerState, Waywin, WaywinState};
use crate::event::{WaywinEvent, WindowEvent};
use raw_window_handle as rwh;
use std::{
    ptr::NonNull,
    sync::{Arc, Mutex, Weak},
};
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_callback::{self, WlCallback},
        wl_surface::{self, WlSurface},
    },
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::{
    wp::{
        fractional_scale::v1::client::wp_fractional_scale_v1::{self, WpFractionalScaleV1},
        pointer_constraints::zv1::client::{
            zwp_confined_pointer_v1::ZwpConfinedPointerV1,
            zwp_locked_pointer_v1::ZwpLockedPointerV1, zwp_pointer_constraints_v1::Lifetime,
        },
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

pub struct WindowState {
    surface: WlSurface,
    xdg_surface: XdgSurface,
    toplevel: XdgToplevel,

    // make sure `XdgWmBase` isn't destroyed while this window is alive
    _xdg_base: Arc<super::state::OwnedXdgWmBase>,

    pub state: State,
    pub prev_state: State,
    configure: PendingConfigure,

    // title: String,
    fullscreen: bool,

    redraw: bool,

    locked_pointer: Option<ZwpLockedPointerV1>,
    confined_pointer: Option<ZwpConfinedPointerV1>,

    viewport_scaling: Option<(WpViewport, WpFractionalScaleV1)>,
    decoration: Option<ZxdgToplevelDecorationV1>,
}
impl WindowState {
    pub fn reset_redraw(&mut self) -> bool {
        let redraw = self.redraw;
        self.redraw = false;
        redraw
    }
    pub fn id(&self) -> usize {
        self.surface.id().as_ptr() as usize
    }
    pub fn unlock_pointer(&mut self) {
        if let Some(locked_pointer) = self.locked_pointer.take() {
            locked_pointer.destroy();
        }
    }
    pub fn unconfine_pointer(&mut self) {
        if let Some(confined_pointer) = self.confined_pointer.take() {
            confined_pointer.destroy();
        }
    }
}
impl Drop for WindowState {
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
    state: Arc<Mutex<WindowState>>,

    signal: calloop::LoopSignal,

    pointer_state: Arc<Mutex<PointerState>>,

    qhandle: QueueHandle<WaywinState>,

    // for HasDisplayHandle
    connection: Connection,
    // for id and HasWindowHandle
    surface: WlSurface,
}
impl Window {
    pub fn new(waywin: &mut Waywin, title: &str) -> Result<Self, String> {
        let freeze = waywin.state.qhandle.freeze();

        let state = Arc::new_cyclic(|weak| {
            let surface = {
                waywin
                    .state
                    .compositor
                    .create_surface(&waywin.state.qhandle, weak.clone())
            };
            let xdg_surface = waywin.state.xdg_wm_base.get_xdg_surface(
                &surface,
                &waywin.state.qhandle,
                weak.clone(),
            );
            let toplevel = xdg_surface.get_toplevel(&waywin.state.qhandle, weak.clone());
            toplevel.set_title(title.to_owned());
            toplevel.set_app_id(waywin.state.app_id.clone());

            let decoration = waywin.state.decoration.as_ref().map(|decoration| {
                let decor =
                    decoration.get_toplevel_decoration(&toplevel, &waywin.state.qhandle, ());
                decor.set_mode(Mode::ServerSide);
                decor
            });

            let viewport_scaling = waywin
                .state
                .viewporter
                .as_ref()
                .zip(waywin.state.scaling.as_ref())
                .map(|(viewporter, scaling)| {
                    (
                        viewporter.get_viewport(&surface, &waywin.state.qhandle, ()),
                        scaling.get_fractional_scale(&surface, &waywin.state.qhandle, weak.clone()),
                    )
                });

            let state = State {
                size: (800, 600),
                scale: 1.0,
            };

            Mutex::new(WindowState {
                surface,
                xdg_surface,
                toplevel,
                _xdg_base: waywin.state.xdg_wm_base.clone(),
                state,
                prev_state: state,
                configure: PendingConfigure { size: None },
                redraw: true,
                fullscreen: false,
                locked_pointer: None,
                confined_pointer: None,
                viewport_scaling,
                decoration,
            })
        });
        let surface = state.lock().unwrap().surface.clone();
        let weak = Arc::downgrade(&state);

        surface.commit();

        waywin.state.windows.push(weak.clone());

        drop(freeze);

        Ok(Self {
            surface,
            state,
            qhandle: waywin.state.qhandle.clone(),
            pointer_state: waywin.state.pointer_state.clone(),
            connection: waywin.state.connection.clone(),
            signal: waywin.event_loop.get_signal(),
        })
    }
}
impl Window {
    pub fn get_physical_size(&self) -> (u32, u32) {
        self.state.lock().unwrap().state.physical_size()
    }
    pub fn get_logical_size(&self) -> (f64, f64) {
        self.state.lock().unwrap().state.logical_size()
    }
    pub fn get_scale(&self) -> f64 {
        self.state.lock().unwrap().state.scale
    }
    pub fn set_title(&self, title: &str) {
        self.state
            .lock()
            .unwrap()
            .toplevel
            .set_title(title.to_owned());
    }
    pub fn request_redraw(&self) {
        self.state.lock().unwrap().redraw = true;
        self.signal.wakeup();
    }
    pub fn set_fullscreen(&self, fullscreen: bool) {
        let mut state = self.state.lock().unwrap();
        if fullscreen {
            state.toplevel.set_fullscreen(None);
        } else {
            state.toplevel.unset_fullscreen();
        }
        state.fullscreen = fullscreen;
    }
    pub fn get_fullscreen(&self) -> bool {
        self.state.lock().unwrap().fullscreen
    }

    pub fn lock_pointer(&self) {
        let pointer_state = self.pointer_state.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        if let Some((pointer_constraints, pointer)) = pointer_state
            .pointer_constraints
            .as_ref()
            .zip(pointer_state.pointer.as_ref())
        {
            state.unlock_pointer();
            state.unconfine_pointer();
            let locked_pointer = pointer_constraints.lock_pointer(
                &self.surface,
                pointer,
                None,
                Lifetime::Persistent,
                &self.qhandle,
                (),
            );
            state.locked_pointer = Some(locked_pointer);
        }
    }
    pub fn unlock_pointer(&self) {
        self.state.lock().unwrap().unlock_pointer();
    }
    pub fn is_pointer_locked(&self) -> bool {
        self.state.lock().unwrap().locked_pointer.is_some()
    }

    pub fn confine_pointer(&self) {
        let pointer_state = self.pointer_state.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        if let Some((pointer_constraints, pointer)) = pointer_state
            .pointer_constraints
            .as_ref()
            .zip(pointer_state.pointer.as_ref())
        {
            state.unconfine_pointer();
            state.unlock_pointer();
            let confined_pointer = pointer_constraints.confine_pointer(
                &self.surface,
                pointer,
                None,
                Lifetime::Persistent,
                &self.qhandle,
                (),
            );
            state.confined_pointer = Some(confined_pointer);
        }
    }
    pub fn unconfine_pointer(&self) {
        self.state.lock().unwrap().unconfine_pointer();
    }
    pub fn is_pointer_confined(&self) -> bool {
        self.state.lock().unwrap().confined_pointer.is_some()
    }

    pub fn id(&self) -> usize {
        self.surface.id().as_ptr() as usize
    }
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

delegate_noop!(WaywinState: WpViewport);
delegate_noop!(WaywinState: ignore ZxdgToplevelDecorationV1);
delegate_noop!(WaywinState: ignore ZwpLockedPointerV1);
delegate_noop!(WaywinState: ignore ZwpConfinedPointerV1);

impl Dispatch<WlSurface, Weak<Mutex<WindowState>>> for WaywinState {
    fn event(
        _state: &mut Self,
        proxy: &WlSurface,
        event: <WlSurface as wayland_client::Proxy>::Event,
        data: &Weak<Mutex<WindowState>>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let Some(data) = data.upgrade() else {
            return;
        };
        let mut data = data.lock().unwrap();

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
                proxy.set_buffer_scale(factor as i32);
                data.state.scale = factor;
            }
            wl_surface::Event::PreferredBufferTransform { transform: _ } => {}
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<WlCallback, Weak<Mutex<WindowState>>> for WaywinState {
    fn event(
        _state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as wayland_client::Proxy>::Event,
        data: &Weak<Mutex<WindowState>>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let Some(_data) = data.upgrade() else {
            return;
        };
        match event {
            wl_callback::Event::Done { callback_data: _ } => {
                todo!()
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<XdgSurface, Weak<Mutex<WindowState>>> for WaywinState {
    fn event(
        _state: &mut Self,
        proxy: &XdgSurface,
        event: <XdgSurface as wayland_client::Proxy>::Event,
        data: &Weak<Mutex<WindowState>>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            xdg_surface::Event::Configure { serial } => {
                proxy.ack_configure(serial);

                let Some(data) = data.upgrade() else {
                    return;
                };
                let mut data = data.lock().unwrap();

                match data.configure.size {
                    Some(configure_size) => {
                        data.state.size = configure_size;
                    }
                    None => data.configure.size = Some(data.state.size),
                }
                if let Some((viewport, _)) = &data.viewport_scaling {
                    viewport.set_destination(data.state.size.0, data.state.size.1);
                }
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<XdgToplevel, Weak<Mutex<WindowState>>> for WaywinState {
    fn event(
        state: &mut Self,
        _proxy: &XdgToplevel,
        event: <XdgToplevel as wayland_client::Proxy>::Event,
        data: &Weak<Mutex<WindowState>>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let Some(data) = data.upgrade() else {
            return;
        };
        let mut data = data.lock().unwrap();

        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states: _,
            } => {
                if !(width == 0 || height == 0) {
                    data.configure.size = Some((width, height))
                } else {
                    data.configure.size = None;
                }
            }
            xdg_toplevel::Event::Close => {
                state.events.push(WaywinEvent::WindowEvent {
                    event: WindowEvent::Close,
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
impl Dispatch<WpFractionalScaleV1, Weak<Mutex<WindowState>>> for WaywinState {
    fn event(
        _state: &mut Self,
        _proxy: &WpFractionalScaleV1,
        event: <WpFractionalScaleV1 as Proxy>::Event,
        data: &Weak<Mutex<WindowState>>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let Some(data) = data.upgrade() else {
            return;
        };
        let mut data = data.lock().unwrap();

        match event {
            wp_fractional_scale_v1::Event::PreferredScale { scale } => {
                let scale = scale as f64 / 120.0;

                data.state.scale = scale;
            }
            _ => unimplemented!(),
        }
    }
}
