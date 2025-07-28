use crate::{
    event::{WaywinEvent, WindowEvent},
    wayland_impl::state::WaywinState,
};
use raw_window_handle as rwh;
use std::ptr::NonNull;
use wayland_client::Proxy;
pub use window::Window;

mod state;
mod window;

pub struct Waywin {
    state: WaywinState,

    event_loop: calloop::EventLoop<'static, WaywinState>,
}
impl Waywin {
    pub fn init(instance: &str) -> Result<Self, String> {
        let event_loop = calloop::EventLoop::try_new()
            .map_err(|err| format!("failed to create polling system: {err}"))?;

        let (state, event_queue) = WaywinState::new(instance, event_loop.handle())?;

        calloop_wayland_source::WaylandSource::new(state.connection.clone(), event_queue)
            .insert(event_loop.handle())
            .map_err(|err| err.to_string())?;

        Ok(Self { state, event_loop })
    }
    pub fn run(&mut self, mut event_hook: impl FnMut(WaywinEvent, &mut bool) + 'static) {
        let mut running = true;
        let signal = self.event_loop.get_signal();

        self.event_loop
            .run(None, &mut self.state, |state| {
                state.windows.retain(|window| {
                    if let Some(window) = window.upgrade() {
                        let curr_state = window.state.lock().unwrap();
                        let mut prev_state = window.prev_state.lock().unwrap();

                        let scaled = prev_state.scale != curr_state.scale;
                        let resized = prev_state.size != curr_state.size;
                        *prev_state = *curr_state;

                        drop(curr_state);
                        drop(prev_state);

                        if scaled {
                            state.events.push(WaywinEvent::WindowEvent {
                                event: WindowEvent::NewScaleFactor,
                                window_id: window.id(),
                            });
                        }
                        if resized || scaled {
                            state.events.push(WaywinEvent::WindowEvent {
                                event: WindowEvent::Resized,
                                window_id: window.id(),
                            });
                        }

                        if window.reset_redraw() || resized || scaled {
                            state.events.push(WaywinEvent::WindowEvent {
                                event: WindowEvent::Paint,
                                window_id: window.id(),
                            });
                        }
                        true
                    } else {
                        false
                    }
                });

                for event in state.events.drain(..) {
                    event_hook(event, &mut running);
                    if !running {
                        signal.stop();
                        signal.wakeup();
                        return;
                    }
                }
            })
            .unwrap();
    }
}

impl rwh::HasDisplayHandle for Waywin {
    fn display_handle(&self) -> std::result::Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let ptr = self.state.connection.display().id().as_ptr();
        let handle = rwh::WaylandDisplayHandle::new(NonNull::new(ptr as *mut _).unwrap());
        unsafe { Ok(rwh::DisplayHandle::borrow_raw(handle.into())) }
    }
}
