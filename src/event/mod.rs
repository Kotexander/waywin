use smol_str::SmolStr;

mod keyboard;
pub use keyboard::*;

mod pointer;
pub use pointer::*;

#[derive(Debug, Clone)]
pub enum WindowEvent {
    Paint,
    Close,
    Resized,
    NewScaleFactor,
    Focus(bool),
    Key {
        down: bool,
        physical_key: PhysicalKey,
        logical_key: LogicalKey,
        text: SmolStr,
        text_raw: SmolStr,
        logical_key_unmodified: LogicalKey,
    },
    PointerEntered,
    PointerLeft,
    /// In logical pixels.
    PointerMoved(f64, f64),
    PointerButton {
        down: bool,
        button: PointerButton,
    },
    Scroll {
        direction: ScrollDirection,
        value: f64,
    },
    // KeyModifiers(KeyModifiers),
}

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    PointerMoved {
        delta: (f64, f64),
        delta_unaccel: (f64, f64),
    },
}

#[derive(Debug, Clone)]
pub enum WaywinEvent {
    WindowEvent {
        event: WindowEvent,
        window_id: usize,
    },
    DeviceEvent(DeviceEvent),
}
