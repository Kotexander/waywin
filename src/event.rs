#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Paint,
    Close,
    Resized,
    NewScaleFactor,
    Focus(bool),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowEvent {
    pub kind: Event,
    pub window_id: usize,
}
