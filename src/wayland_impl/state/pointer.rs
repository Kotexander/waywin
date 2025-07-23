use super::WaywinState;
use crate::event::{Event, PointerButton, WindowEvent};
use wayland_client::{
    protocol::wl_pointer::{ButtonState, WlPointer},
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
};
use wayland_protocols::wp::relative_pointer::zv1::client::{
    zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
    zwp_relative_pointer_v1::ZwpRelativePointerV1,
};

#[derive(Default)]
pub struct PointerState {
    pub pointer: Option<WlPointer>,
    pub relative_pointer: Option<ZwpRelativePointerV1>,
    pub relative_pointer_manager: Option<ZwpRelativePointerManagerV1>,
    pub focused_window: Option<usize>,
}

impl Dispatch<WlPointer, ()> for WaywinState {
    fn event(
        state: &mut Self,
        _proxy: &WlPointer,
        event: <WlPointer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("{event:?}");

        match event {
            wayland_client::protocol::wl_pointer::Event::Enter {
                serial: _,
                surface,
                surface_x,
                surface_y,
            } => {
                if let Some(id) = state.pointer.focused_window.take() {
                    log::warn!("pointer entered new window before leaving old window");
                    state.events.push(WindowEvent {
                        kind: Event::PointerLeft,
                        window_id: id,
                    });
                }
                let id = surface.id().as_ptr() as usize;
                state.pointer.focused_window = Some(id);
                state.events.push(WindowEvent {
                    kind: Event::PointerEntered,
                    window_id: id,
                });
                state.events.push(WindowEvent {
                    kind: Event::PointerMoved(surface_x, surface_y),
                    window_id: id,
                });
            }
            wayland_client::protocol::wl_pointer::Event::Leave { serial: _, surface } => {
                let id = surface.id().as_ptr() as usize;
                if Some(id) != state.pointer.focused_window {
                    log::warn!("pointer leaving unfocused window: {id}");
                } else {
                    state.pointer.focused_window = None;
                    state.events.push(WindowEvent {
                        kind: Event::PointerLeft,
                        window_id: id,
                    });
                }
            }
            wayland_client::protocol::wl_pointer::Event::Motion {
                time: _,
                surface_x,
                surface_y,
            } => {
                let Some(id) = state.pointer.focused_window else {
                    log::warn!("recieved a pointer motion event while no window is focused");
                    return;
                };
                state.events.push(WindowEvent {
                    kind: Event::PointerMoved(surface_x, surface_y),
                    window_id: id,
                });
            }
            wayland_client::protocol::wl_pointer::Event::Button {
                serial: _,
                time: _,
                button,
                state: WEnum::Value(ButtonState::Pressed),
            } => {
                let Some(id) = state.pointer.focused_window else {
                    log::warn!("recieved a pointer button down event while no window is focused");
                    return;
                };
                state.events.push(WindowEvent {
                    kind: Event::PointerButton {
                        down: true,
                        button: PointerButton::from(button),
                    },
                    window_id: id,
                });
            }
            wayland_client::protocol::wl_pointer::Event::Button {
                serial: _,
                time: _,
                button,
                state: WEnum::Value(ButtonState::Released),
            } => {
                let Some(id) = state.pointer.focused_window else {
                    log::warn!("recieved a pointer button up event while no window is focused");
                    return;
                };
                state.events.push(WindowEvent {
                    kind: Event::PointerButton {
                        down: false,
                        button: PointerButton::from(button),
                    },
                    window_id: id,
                });
            }
            wayland_client::protocol::wl_pointer::Event::Button {
                serial: _,
                time: _,
                button: _,
                state: WEnum::Unknown(_),
            } => {
                log::error!("unknown pointer button state sent by OS")
            }
            // wayland_client::protocol::wl_pointer::Event::Axis { time, axis, value } => todo!(),
            wayland_client::protocol::wl_pointer::Event::Frame => {
                // TODO: maybe collect pointer events into a frame
            }
            // wayland_client::protocol::wl_pointer::Event::AxisSource { axis_source } => todo!(),
            // wayland_client::protocol::wl_pointer::Event::AxisStop { time, axis } => todo!(),
            // wayland_client::protocol::wl_pointer::Event::AxisDiscrete { axis, discrete } => todo!(),
            // wayland_client::protocol::wl_pointer::Event::AxisValue120 { axis, value120 } => todo!(),
            // wayland_client::protocol::wl_pointer::Event::AxisRelativeDirection { axis, direction } => todo!(),
            _ => { //todo
            }
        }
    }
}

impl Dispatch<ZwpRelativePointerV1, ()> for WaywinState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpRelativePointerV1,
        event: <ZwpRelativePointerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("{event:?}");
    }
}

impl From<u32> for PointerButton {
    fn from(value: u32) -> Self {
        match value {
            0x110 => Self::Left,
            0x111 => Self::Right,
            0x112 => Self::Middle,
            0x114 => Self::Forward,
            0x113 => Self::Back,
            _ => Self::Unknown(value),
        }
    }
}
