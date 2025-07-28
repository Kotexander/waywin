use super::WaywinState;
use crate::event::{DeviceEvent, PointerButton, ScrollDirection, WaywinEvent, WindowEvent};
use wayland_client::{
    protocol::wl_pointer::{Axis, ButtonState, WlPointer},
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
};
use wayland_protocols::wp::relative_pointer::zv1::client::{
    zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
    zwp_relative_pointer_v1::{self, ZwpRelativePointerV1},
};

#[derive(Default)]
// members are released by `WaywinState`
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
        match event {
            wayland_client::protocol::wl_pointer::Event::Enter {
                serial: _,
                surface,
                surface_x,
                surface_y,
            } => {
                if let Some(id) = state.pointer.focused_window.take() {
                    log::warn!("pointer entered new window before leaving old window");
                    state.events.push(WaywinEvent::WindowEvent {
                        event: WindowEvent::PointerLeft,
                        window_id: id,
                    });
                }
                let id = surface.id().as_ptr() as usize;
                state.pointer.focused_window = Some(id);
                state.events.push(WaywinEvent::WindowEvent {
                    event: WindowEvent::PointerEntered,
                    window_id: id,
                });
                state.events.push(WaywinEvent::WindowEvent {
                    event: WindowEvent::PointerMoved(surface_x, surface_y),
                    window_id: id,
                });
            }
            wayland_client::protocol::wl_pointer::Event::Leave { serial: _, surface } => {
                let id = surface.id().as_ptr() as usize;
                if Some(id) != state.pointer.focused_window {
                    log::warn!("pointer leaving unfocused window: {id}");
                } else {
                    state.pointer.focused_window = None;
                    state.events.push(WaywinEvent::WindowEvent {
                        event: WindowEvent::PointerLeft,
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
                state.events.push(WaywinEvent::WindowEvent {
                    event: WindowEvent::PointerMoved(surface_x, surface_y),
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
                state.events.push(WaywinEvent::WindowEvent {
                    event: WindowEvent::PointerButton {
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
                state.events.push(WaywinEvent::WindowEvent {
                    event: WindowEvent::PointerButton {
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
            wayland_client::protocol::wl_pointer::Event::Axis {
                time: _,
                axis: WEnum::Value(axis),
                value,
            } => {
                let Some(id) = state.pointer.focused_window else {
                    log::warn!("recieved a pointer scroll event while no window is focused");
                    return;
                };
                state.events.push(WaywinEvent::WindowEvent {
                    event: WindowEvent::Scroll {
                        direction: ScrollDirection::from(axis),
                        value: -value,
                    },
                    window_id: id,
                });
            }
            wayland_client::protocol::wl_pointer::Event::Axis {
                time: _,
                axis: WEnum::Unknown(_),
                value: _,
            } => {
                log::error!("unknown pointer scroll axis sent by OS")
            }
            wayland_client::protocol::wl_pointer::Event::Frame => {
                // TODO: maybe collect pointer events into a frame
            }
            wayland_client::protocol::wl_pointer::Event::AxisSource { .. } => {}
            wayland_client::protocol::wl_pointer::Event::AxisStop { .. } => {}
            wayland_client::protocol::wl_pointer::Event::AxisDiscrete { .. } => {}
            wayland_client::protocol::wl_pointer::Event::AxisValue120 { .. } => {}
            wayland_client::protocol::wl_pointer::Event::AxisRelativeDirection { .. } => {}
            _ => {
                unimplemented!()
            }
        }
    }
}

impl Dispatch<ZwpRelativePointerV1, ()> for WaywinState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpRelativePointerV1,
        event: <ZwpRelativePointerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwp_relative_pointer_v1::Event::RelativeMotion {
                utime_hi: _,
                utime_lo: _,
                dx,
                dy,
                dx_unaccel,
                dy_unaccel,
            } => {
                state
                    .events
                    .push(WaywinEvent::DeviceEvent(DeviceEvent::PointerMoved {
                        delta: (dx, dy),
                        delta_unaccel: (dx_unaccel, dy_unaccel),
                    }));
            }
            _ => unimplemented!(),
        }
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

impl From<Axis> for ScrollDirection {
    fn from(value: Axis) -> Self {
        match value {
            Axis::VerticalScroll => Self::Vertical,
            Axis::HorizontalScroll => Self::Horizontal,
            _ => unimplemented!(),
        }
    }
}
