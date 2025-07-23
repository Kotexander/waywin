use smol_str::SmolStr;

mod keyboard;
pub use keyboard::*;

#[derive(Debug, Clone)]
pub enum Event {
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
    PointerMoved(f64, f64),
    PointerButton {
        down: bool,
        button: PointerButton,
    },
    // KeyModifiers(KeyModifiers),
}

#[derive(Debug, Clone, Copy)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    Forward,
    Back,
    Unknown(u32),
}

#[derive(Debug, Clone)]
pub struct WindowEvent {
    pub kind: Event,
    pub window_id: usize,
}
