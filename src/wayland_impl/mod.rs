use crate::event::{Event, WindowEvent};
use std::sync::Weak;
use wayland_client::{
    globals::registry_queue_init,
    protocol::{
        wl_compositor::WlCompositor, wl_keyboard::WlKeyboard, wl_pointer::WlPointer,
        wl_seat::WlSeat,
    },
    Connection, QueueHandle,
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
        shell::client::xdg_wm_base::XdgWmBase,
    },
};
pub use window::Window;
use window::WindowInner;

mod proxy;
mod window;

#[derive(Default)]
struct WaywinState {
    pointer: Option<WlPointer>,
    relative_pointer: Option<ZwpRelativePointerV1>,
    keyboard: Option<WlKeyboard>,
    relative_pointer_manager: Option<ZwpRelativePointerManagerV1>,
}
impl Drop for WaywinState {
    fn drop(&mut self) {
        if let Some(s) = self.pointer.take() {
            s.release()
        }
        if let Some(s) = self.relative_pointer.take() {
            s.destroy()
        }
        if let Some(s) = self.keyboard.take() {
            s.release()
        }
        if let Some(s) = self.relative_pointer_manager.take() {
            s.destroy()
        }
    }
}

pub struct Waywin {
    compositor: WlCompositor,
    xdg_wm_base: XdgWmBase,
    seat: WlSeat,
    decoration: Option<ZxdgDecorationManagerV1>,
    viewporter: Option<WpViewporter>,
    scaling: Option<WpFractionalScaleManagerV1>,

    // event_queue: EventQueue<WaywinState>,
    event_loop: calloop::EventLoop<'static, WaywinState>,
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

        let event_loop = calloop::EventLoop::try_new().unwrap();
        calloop_wayland_source::WaylandSource::new(connection.clone(), event_queue)
            .insert(event_loop.handle())
            .unwrap();

        Ok(Self {
            compositor,
            xdg_wm_base,
            seat,
            decoration,
            viewporter,
            scaling,

            // event_queue,
            event_loop,
            connection,
            state,
            qhandle,
            app_id: instance.to_owned(),
            windows: vec![],
        })
    }
    pub fn run(&mut self, mut event_hook: impl FnMut(WindowEvent) + 'static) {
        self.event_loop
            .run(None, &mut self.state, |_| {
                self.windows.retain(|window| {
                    if let Some(window) = window.upgrade() {
                        let state = window.state.lock().unwrap();
                        let mut prev_state = window.prev_state.lock().unwrap();

                        let scaled = prev_state.scale != state.scale;
                        let resized = prev_state.size != state.size;
                        *prev_state = *state;

                        let size = state.size;

                        drop(state);
                        drop(prev_state);

                        if scaled {
                            event_hook(WindowEvent {
                                kind: Event::NewScaleFactor,
                                window_id: window.id(),
                            });
                        }
                        if resized || scaled {
                            if let Some((viewport, _)) = &window.viewport_scaling {
                                viewport.set_destination(size.0, size.1);
                            }
                            event_hook(WindowEvent {
                                kind: Event::Resized,
                                window_id: window.id(),
                            });
                        }

                        if window.reset_redraw() || resized || scaled {
                            event_hook(WindowEvent {
                                kind: Event::Paint,
                                window_id: window.id(),
                            });
                        }
                        true
                    } else {
                        false
                    }
                });
            })
            .unwrap();
    }
}
impl Drop for Waywin {
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
        self.xdg_wm_base.destroy(); // TODO: don't destroy while windows are up
    }
}
impl std::fmt::Debug for Waywin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waywin").finish_non_exhaustive()
    }
}
