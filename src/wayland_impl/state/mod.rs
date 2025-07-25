use super::window::WindowInner;
use crate::event::WindowEvent;
use keyboard::KeyboardState;
use pointer::PointerState;
use std::sync::Weak;
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
mod pointer;
mod proxy;

pub struct WaywinState {
    pub compositor: WlCompositor,
    pub xdg_wm_base: XdgWmBase,
    pub seat: WlSeat,
    pub decoration: Option<ZxdgDecorationManagerV1>,
    pub viewporter: Option<WpViewporter>,
    pub scaling: Option<WpFractionalScaleManagerV1>,

    pub keyboard: KeyboardState,
    pub pointer: PointerState,

    pub qhandle: QueueHandle<Self>,
    pub connection: Connection,
    pub app_id: String,

    pub windows: Vec<Weak<WindowInner>>,
    pub handle: calloop::LoopHandle<'static, Self>,

    pub events: Vec<WindowEvent>,
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

        Ok((
            Self {
                compositor,
                xdg_wm_base,
                seat,
                decoration,
                viewporter,
                scaling,

                pointer: PointerState {
                    relative_pointer_manager,
                    ..Default::default()
                },
                keyboard: KeyboardState::default(),

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
        if let Some(s) = self.pointer.pointer.take() {
            s.release()
        }
        if let Some(s) = self.pointer.relative_pointer.take() {
            s.destroy()
        }
        if let Some(s) = self.keyboard.keyboard.take() {
            s.release()
        }
        if let Some(s) = self.pointer.relative_pointer_manager.take() {
            s.destroy()
        }

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
        self.xdg_wm_base.destroy(); // TODO: don't destroy while windows are up
    }
}
