use crate::{
    event::{Event, WindowEvent},
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
        let event_loop = calloop::EventLoop::try_new().unwrap();

        let (state, event_queue) = WaywinState::new(instance, event_loop.handle())?;

        calloop_wayland_source::WaylandSource::new(state.connection.clone(), event_queue)
            .insert(event_loop.handle())
            .unwrap();

        Ok(Self { state, event_loop })
    }
    pub fn run(&mut self, mut event_hook: impl FnMut(WindowEvent) + 'static) {
        self.event_loop
            .run(None, &mut self.state, |state| {
                // TODO: maybe check if the window hasn't been droped before sending the events
                for event in state.events.drain(..) {
                    event_hook(event);
                }

                state.windows.retain(|window| {
                    if let Some(window) = window.upgrade() {
                        let state = window.state.lock().unwrap();
                        let mut prev_state = window.prev_state.lock().unwrap();

                        let scaled = prev_state.scale != state.scale;
                        let resized = prev_state.size != state.size;
                        *prev_state = *state;

                        drop(state);
                        drop(prev_state);

                        if scaled {
                            event_hook(WindowEvent {
                                kind: Event::NewScaleFactor,
                                window_id: window.id(),
                            });
                        }
                        if resized || scaled {
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

impl std::fmt::Debug for Waywin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waywin").finish_non_exhaustive()
    }
}

impl rwh::HasDisplayHandle for Waywin {
    fn display_handle(&self) -> std::result::Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let ptr = self.state.connection.display().id().as_ptr();
        let handle = rwh::WaylandDisplayHandle::new(NonNull::new(ptr as *mut _).unwrap());
        unsafe { Ok(rwh::DisplayHandle::borrow_raw(handle.into())) }
    }
}
