use crate::event::WaywinEvent;
use keyboard::KeyboardState;
use pointer::PointerState;
use std::{
    ops::Deref,
    sync::{Arc, Mutex, Weak},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_compositor::WlCompositor, wl_seat::WlSeat},
    Connection, EventQueue, QueueHandle,
};
use wayland_protocols::{
    wp::{
        fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        viewporter::client::wp_viewporter::WpViewporter,
    },
    xdg::{
        decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        shell::client::xdg_wm_base::XdgWmBase,
    },
};

mod keyboard;
pub mod pointer;
mod proxy;

pub struct WaywinState {
    pub compositor: WlCompositor,
    pub xdg_wm_base: Arc<OwnedXdgWmBase>,
    pub seat: WlSeat,
    pub decoration: Option<ZxdgDecorationManagerV1>,
    pub viewporter: Option<WpViewporter>,
    pub scaling: Option<WpFractionalScaleManagerV1>,

    pub keyboard_state: KeyboardState,
    pub pointer_state: Arc<Mutex<PointerState>>,

    pub qhandle: QueueHandle<Self>,
    pub connection: Connection,
    pub app_id: String,

    pub windows: Vec<Weak<Mutex<super::window::WindowState>>>,
    pub handle: calloop::LoopHandle<'static, Self>,

    pub events: Vec<WaywinEvent>,
}
impl WaywinState {
    pub fn new(
        instance: &str,
        handle: calloop::LoopHandle<'static, Self>,
    ) -> Result<(Self, EventQueue<Self>), String> {
        let connection = Connection::connect_to_env()
            .map_err(|err| format!("failed to connect to wayland: {err}"))?;

        let (globals, event_queue) = registry_queue_init(&connection).unwrap();

        let qhandle = event_queue.handle();

        let compositor = globals
            .bind(&qhandle, 1..=6, ())
            .map_err(|err| format!("failed to bind WlCompositor: {err}"))?;
        let xdg_wm_base = globals
            .bind(&qhandle, 1..=7, ())
            .map_err(|err| format!("failed to bind XdgWmBase: {err}"))?;
        let seat = globals
            .bind(&qhandle, 1..=9, ())
            .map_err(|err| format!("failed to bind WlSeat: {err}"))?;
        let decoration = globals.bind(&qhandle, 1..=1, ()).ok();
        let viewporter = globals.bind(&qhandle, 1..=1, ()).ok();
        let scaling = globals.bind(&qhandle, 1..=1, ()).ok();

        let relative_pointer_manager = globals.bind(&qhandle, 1..=1, ()).ok();
        let pointer_constraints = globals.bind(&qhandle, 1..=1, ()).ok();

        Ok((
            Self {
                compositor,
                xdg_wm_base: Arc::new(OwnedXdgWmBase(xdg_wm_base)),
                seat,
                decoration,
                viewporter,
                scaling,

                pointer_state: Arc::new(Mutex::new(PointerState {
                    pointer: None,
                    relative_pointer: None,
                    focused_window: None,
                    relative_pointer_manager,
                    pointer_constraints,
                })),
                keyboard_state: KeyboardState::default(),

                connection,
                qhandle,
                app_id: instance.to_owned(),
                windows: vec![],
                handle,
                events: vec![],
            },
            event_queue,
        ))
    }
}
impl Drop for WaywinState {
    fn drop(&mut self) {
        if let Some(s) = self.scaling.take() {
            s.destroy();
        }
        if let Some(s) = self.viewporter.take() {
            s.destroy();
        }
        if let Some(s) = self.decoration.take() {
            s.destroy();
        }
        self.seat.release();

        // should be destroyed automatically when it can
        // self.xdg_wm_base.destroy();
    }
}

/// Used to make sure `XdgWmBase` is not destroyed while windows are up.
/// Needs to be referenced counted by something else.
#[derive(Clone)]
pub struct OwnedXdgWmBase(XdgWmBase);
impl Drop for OwnedXdgWmBase {
    fn drop(&mut self) {
        self.0.destroy();
    }
}
impl Deref for OwnedXdgWmBase {
    type Target = XdgWmBase;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
