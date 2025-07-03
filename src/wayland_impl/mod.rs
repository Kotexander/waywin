use std::sync::Arc;
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_compositor::WlCompositor,
        wl_registry::{self, WlRegistry},
    },
    Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};
pub use window::Window;

use crate::event::WindowEvent;

mod window;

#[derive(Default)]
struct State {
    compositor: Option<WlCompositor>,
    xdg_wm_base: Option<XdgWmBase>,
    event_hook: Option<Box<dyn FnMut(WindowEvent)>>,
    running: bool,
}
impl State {
    pub fn hook(&mut self, event: WindowEvent) {
        if let Some(hook) = &mut self.event_hook {
            hook(event);
        }
    }
}
impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        log::info!("WlRegistry {event:?}");

        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            match &interface[..] {
                "wl_compositor" => {
                    state.compositor = Some(proxy.bind(name, 1, qhandle, ()));
                }
                "xdg_wm_base" => {
                    state.xdg_wm_base = Some(proxy.bind(name, 1, qhandle, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<XdgWmBase, ()> for State {
    fn event(
        _state: &mut Self,
        proxy: &XdgWmBase,
        event: <XdgWmBase as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::info!("XdgWmBase {event:?}");

        match event {
            xdg_wm_base::Event::Ping { serial } => {
                proxy.pong(serial);
            }
            _ => unreachable!(),
        }
    }
}

delegate_noop!(State: WlCompositor);

pub struct Waywin {
    event_queue: EventQueue<State>,
    qhandle: QueueHandle<State>,
    connection: Arc<Connection>,
    state: State,
}
impl Waywin {
    pub fn init(instance: &str) -> Result<Self, String> {
        let connection = Connection::connect_to_env()
            .map_err(|err| format!("failed to connect to wayland: {err}"))?;

        let mut simple = State::default();

        // discover interfaces
        let mut event_queue = connection.new_event_queue::<State>();
        let qhandle = event_queue.handle();
        let display = connection.display();
        display.get_registry(&qhandle, ());

        event_queue.roundtrip(&mut simple).unwrap();

        log::info!("Init {instance} done");

        Ok(Self {
            event_queue,
            connection: Arc::new(connection),
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
