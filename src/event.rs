#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Paint,
    Close,
    Resize(u32, u32),
    NewScaleFactor(f64),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowEvent {
    pub kind: Event,
    pub window_id: usize,
}
