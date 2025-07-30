use super::WaywinState;
use wayland_client::{
    delegate_noop,
    globals::GlobalListContents,
    protocol::{
        wl_compositor::WlCompositor,
        wl_registry::{self, WlRegistry},
        wl_seat::{self, Capability, WlSeat},
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::{
    wp::{
        fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        pointer_constraints::zv1::client::zwp_pointer_constraints_v1::ZwpPointerConstraintsV1,
        relative_pointer::zv1::client::zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
        viewporter::client::wp_viewporter::WpViewporter,
    },
    xdg::{
        decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        shell::client::xdg_wm_base::{self, XdgWmBase},
    },
};

impl Dispatch<WlRegistry, GlobalListContents> for WaywinState {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        // TODO: react to dynamic global
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
        let mut pointer_state = state.pointer_state.lock().unwrap();

        match event {
            wl_seat::Event::Capabilities { capabilities } => {
                if let Some(s) = pointer_state.pointer.take() {
                    s.release();
                }
                if let Some(s) = pointer_state.relative_pointer.take() {
                    s.destroy();
                }
                if let Some(s) = state.keyboard_state.keyboard.take() {
                    s.release();
                }
                if let WEnum::Value(cap) = capabilities {
                    if cap.intersects(Capability::Pointer) {
                        pointer_state.pointer = Some(proxy.get_pointer(qhandle, ()));
                        pointer_state.relative_pointer = pointer_state
                            .pointer
                            .as_ref()
                            .zip(pointer_state.relative_pointer_manager.as_ref())
                            .map(|(pointer, manager)| {
                                manager.get_relative_pointer(pointer, qhandle, ())
                            });
                    }
                    if cap.intersects(Capability::Keyboard) {
                        state.keyboard_state.keyboard = Some(proxy.get_keyboard(qhandle, ()));
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

delegate_noop!(WaywinState: WlCompositor);
delegate_noop!(WaywinState: ZxdgDecorationManagerV1);
delegate_noop!(WaywinState: WpViewporter);
delegate_noop!(WaywinState: WpFractionalScaleManagerV1);
delegate_noop!(WaywinState: ZwpRelativePointerManagerV1);
delegate_noop!(WaywinState: ZwpPointerConstraintsV1);
