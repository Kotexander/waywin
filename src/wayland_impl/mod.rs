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
use wayland_protocols::xdg::{
    decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
    shell::client::xdg_wm_base::{self, XdgWmBase},
};
pub use window::Window;

mod window;

#[derive(Default)]
struct WaywinState {
    compositor: Option<WlCompositor>,
    xdg_wm_base: Option<XdgWmBase>,
    decoration: Option<ZxdgDecorationManagerV1>,
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
        log::debug!("XdgWmBase {event:?}");

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

pub struct Waywin {
    event_queue: EventQueue<WaywinState>,
    qhandle: QueueHandle<WaywinState>,
    connection: Connection,
    state: WaywinState,
}
impl Waywin {
    pub fn init(instance: &str) -> Result<Self, String> {
        let connection = Connection::connect_to_env()
            .map_err(|err| format!("failed to connect to wayland: {err}"))?;

        let mut simple = WaywinState::default();

        let (globals, event_queue) = registry_queue_init(&connection).unwrap();

        let qhandle = event_queue.handle();
        simple.compositor = Some(globals.bind(&qhandle, 1..=6, ()).unwrap());
        simple.xdg_wm_base = Some(globals.bind(&qhandle, 1..=7, ()).unwrap());
        simple.decoration = Some(globals.bind(&qhandle, 1..=1, ()).unwrap());

        log::info!("Init {instance} done");

        Ok(Self {
            event_queue,
            connection,
            state: simple,
            qhandle,
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
