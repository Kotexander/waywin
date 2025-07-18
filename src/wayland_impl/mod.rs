use crate::event::{Event, WindowEvent};
use std::sync::Weak;
use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_compositor::WlCompositor,
        wl_keyboard::WlKeyboard,
        wl_pointer::WlPointer,
        wl_registry::{self, WlRegistry},
        wl_seat::{self, Capability, WlSeat},
    },
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols::{
    wp::{
        fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        relative_pointer::zv1::client::{
            zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
            zwp_relative_pointer_v1::ZwpRelativePointerV1,
        },
        viewporter::client::wp_viewporter::WpViewporter,
    },
    xdg::{
        decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        shell::client::xdg_wm_base::{self, XdgWmBase},
    },
};
pub use window::Window;
use window::WindowInner;

mod window;

#[derive(Default)]
struct WaywinState {
    event_hook: Option<Box<dyn FnMut(WindowEvent, &mut bool)>>,
    running: bool,

    pointer: Option<WlPointer>,
    relative_pointer: Option<ZwpRelativePointerV1>,
    keyboard: Option<WlKeyboard>,
    relative_pointer_manager: Option<ZwpRelativePointerManagerV1>,
}
impl WaywinState {
    pub fn hook(&mut self, event: WindowEvent) {
        if let Some(hook) = &mut self.event_hook {
            hook(event, &mut self.running);
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for WaywinState {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        /* react to dynamic global events here */
    }
}
impl Dispatch<XdgWmBase, ()> for WaywinState {
    fn event(
        _state: &mut Self,
        proxy: &XdgWmBase,
        event: <XdgWmBase as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            xdg_wm_base::Event::Ping { serial } => {
                proxy.pong(serial);
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<WlSeat, ()> for WaywinState {
    fn event(
        state: &mut Self,
        proxy: &WlSeat,
        event: <WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_seat::Event::Capabilities { capabilities } => {
                state.pointer = None;
                state.keyboard = None;
                state.relative_pointer = None;
                if let WEnum::Value(cap) = capabilities {
                    if cap.intersects(Capability::Pointer) {
                        state.pointer = Some(proxy.get_pointer(qhandle, ()));
                        state.relative_pointer = state
                            .pointer
                            .as_ref()
                            .zip(state.relative_pointer_manager.as_ref())
                            .map(|(pointer, manager)| {
                                manager.get_relative_pointer(pointer, qhandle, ())
                            });
                    }
                    if cap.intersects(Capability::Keyboard) {
                        state.keyboard = Some(proxy.get_keyboard(qhandle, ()));
                    }
                }
            }
            wl_seat::Event::Name { name: _ } => {
                // TODO
            }
            _ => unimplemented!(),
        }
    }
}
impl Dispatch<WlPointer, ()> for WaywinState {
    fn event(
        state: &mut Self,
        proxy: &WlPointer,
        event: <WlPointer as wayland_client::Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("{event:?}");
    }
}
impl Dispatch<WlKeyboard, ()> for WaywinState {
    fn event(
        state: &mut Self,
        proxy: &WlKeyboard,
        event: <WlKeyboard as wayland_client::Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("{event:?}");
    }
}
impl Dispatch<ZwpRelativePointerV1, ()> for WaywinState {
    fn event(
        state: &mut Self,
        proxy: &ZwpRelativePointerV1,
        event: <ZwpRelativePointerV1 as wayland_client::Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("{event:?}");
    }
}

delegate_noop!(WaywinState: WlCompositor);
delegate_noop!(WaywinState: ZxdgDecorationManagerV1);
delegate_noop!(WaywinState: WpViewporter);
delegate_noop!(WaywinState: WpFractionalScaleManagerV1);
delegate_noop!(WaywinState: ZwpRelativePointerManagerV1);

pub struct Waywin {
    compositor: WlCompositor,
    xdg_wm_base: XdgWmBase,
    seat: WlSeat,
    decoration: Option<ZxdgDecorationManagerV1>,
    viewporter: Option<WpViewporter>,
    scaling: Option<WpFractionalScaleManagerV1>,

    event_queue: EventQueue<WaywinState>,
    qhandle: QueueHandle<WaywinState>,
    connection: Connection,
    state: WaywinState,
    app_id: String,

    windows: Vec<Weak<WindowInner>>,
}
impl Waywin {
    pub fn init(instance: &str) -> Result<Self, String> {
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

        let mut state = WaywinState::default();
        state.relative_pointer_manager = globals.bind(&qhandle, 1..=1, ()).ok();

        Ok(Self {
            compositor,
            xdg_wm_base,
            seat,
            decoration,
            viewporter,
            scaling,

            event_queue,
            connection,
            state,
            qhandle,
            app_id: instance.to_owned(),
            windows: vec![],
        })
    }
    pub fn run(&mut self, event_hook: impl FnMut(WindowEvent, &mut bool) + 'static) {
        self.state.event_hook = Some(Box::new(event_hook));
        self.state.running = true;

        while self.state.running {
            self.event_queue.dispatch_pending(&mut self.state).unwrap();

            self.connection.flush().unwrap();
            let read = self.event_queue.prepare_read().unwrap();
            read.read().unwrap();
            self.event_queue.dispatch_pending(&mut self.state).unwrap();

            self.windows.retain(|window| {
                if let Some(window) = window.upgrade() {
                    // log::info!("{} {}", window.id(), window.)
                    if window.reset_redraw() {
                        self.state.hook(WindowEvent {
                            kind: Event::Paint,
                            window_id: window.id(),
                        });
                    }
                    true
                } else {
                    false
                }
            });
        }
        self.state.event_hook = None;
    }
}

impl std::fmt::Debug for Waywin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waywin").finish_non_exhaustive()
    }
}
