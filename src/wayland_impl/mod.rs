use crate::event::WindowEvent;
use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_compositor::WlCompositor,
        wl_registry::{self, WlRegistry},
    },
    Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols::{
    wp::{
        fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        viewporter::client::wp_viewporter::WpViewporter,
    },
    xdg::{
        decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        shell::client::xdg_wm_base::{self, XdgWmBase},
    },
};
pub use window::Window;

mod window;

#[derive(Default)]
struct WaywinState {
    compositor: Option<WlCompositor>,
    xdg_wm_base: Option<XdgWmBase>,
    decoration: Option<ZxdgDecorationManagerV1>,
    viewporter: Option<WpViewporter>,
    scaling: Option<WpFractionalScaleManagerV1>,
    event_hook: Option<Box<dyn FnMut(WindowEvent)>>,
    running: bool,
}
impl WaywinState {
    pub fn hook(&mut self, event: WindowEvent) {
        if let Some(hook) = &mut self.event_hook {
            hook(event);
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
        // log::debug!("XdgWmBase {event:?}");

        match event {
            xdg_wm_base::Event::Ping { serial } => {
                proxy.pong(serial);
            }
            _ => unimplemented!(),
        }
    }
}

delegate_noop!(WaywinState: WlCompositor);
delegate_noop!(WaywinState: ZxdgDecorationManagerV1);
delegate_noop!(WaywinState: WpViewporter);
delegate_noop!(WaywinState: WpFractionalScaleManagerV1);

pub struct Waywin {
    event_queue: EventQueue<WaywinState>,
    qhandle: QueueHandle<WaywinState>,
    connection: Connection,
    state: WaywinState,
    app_id: String,
}
impl Waywin {
    pub fn init(instance: &str) -> Result<Self, String> {
        let connection = Connection::connect_to_env()
            .map_err(|err| format!("failed to connect to wayland: {err}"))?;

        let mut state = WaywinState::default();

        let (globals, event_queue) = registry_queue_init(&connection).unwrap();

        let qhandle = event_queue.handle();
        state.compositor = Some(
            globals
                .bind(&qhandle, 1..=6, ())
                .map_err(|err| format!("failed to bind WlCompositor: {err}"))?,
        );
        state.xdg_wm_base = Some(
            globals
                .bind(&qhandle, 1..=7, ())
                .map_err(|err| format!("failed to bind XdgWmBase: {err}"))?,
        );
        state.decoration = globals.bind(&qhandle, 1..=1, ()).ok();
        state.viewporter = globals.bind(&qhandle, 1..=1, ()).ok();
        state.scaling = globals.bind(&qhandle, 1..=1, ()).ok();

        Ok(Self {
            event_queue,
            connection,
            state,
            qhandle,
            app_id: instance.to_owned(),
        })
    }
    pub fn exit(&self) {}
    pub fn run(&mut self, event_hook: impl FnMut(WindowEvent) + 'static) {
        self.state.event_hook = Some(Box::new(event_hook));
        self.state.running = true;
        while self.state.running {
            self.event_queue.blocking_dispatch(&mut self.state).unwrap();
        }
        self.state.event_hook = None;
    }
}

impl std::fmt::Debug for Waywin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waywin").finish_non_exhaustive()
    }
}
