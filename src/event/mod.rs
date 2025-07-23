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
    // KeyModifiers(KeyModifiers),
}

#[derive(Debug, Clone)]
pub struct WindowEvent {
    pub kind: Event,
    pub window_id: usize,
}
