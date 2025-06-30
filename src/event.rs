// #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
// pub struct MouseModifier {
//     pub ctrl: bool,
//     pub shift: bool,
//     pub lbtn: bool,
//     pub rbtn: bool,
//     pub mbtn: bool,
//     pub x1btn: bool,
//     pub x2btn: bool,
// }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Paint,
    Close,
    Resize(u32, u32),
    NewScaleFactor(f64),
    MouseMoved(i32, i32),
    Focus(bool),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowEvent {
    pub kind: Event,
    pub window_id: usize,
}
