use smol_str::SmolStr;

#[derive(Debug, Clone, Copy)]
pub enum KeyCode {
    Tab,
    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
    PageUp,
    PageDown,
    Home,
    End,
    Insert,
    Delete,
    Backspace,
    Space,
    Enter,
    Escape,
    LCtrl,
    LShift,
    LAlt,
    LSuper,
    RCtrl,
    RShift,
    RAlt,
    RSuper,
    Menu,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    Numpad0,
    NumpadDecimal,
    NumpadDivide,
    NumpadMultiply,
    NumpadSubtract,
    NumpadAdd,
    NumpadEnter,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Quote,
    Comma,
    Minus,
    Period,
    Slash,
    Backslash,
    Semicolon,
    Equal,
    LBracket,
    RBracket,
    Grave,
    CapsLock,
    ScrollLock,
    NumLock,
    PrintScreen,
    Pause,
}
#[derive(Debug, Clone, Copy)]
pub enum PhysicalKey {
    KeyCode(KeyCode),
    /// OS scancode
    Unknown(u32),
}

#[derive(Debug, Clone, Copy)]
pub enum Key {
    Tab,
    Enter,
    Space,
    Period,

    Shift,
    LShift,
    RShift,

    Ctrl,
    LCtrl,
    RCtrl,

    Super,
    LSuper,
    RSuper,

    Alt,
    LAlt,
    RAlt,

    CapsLock,
    NumLock,
    ScrollLock,
    PrintScreen,

    Backspace,
    Escape,

    Pause,
    Menu,

    PageUp,
    PageDown,
    End,
    Home,
    Delete,
    Insert,

    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,

    Plus,
    Minus,
    Asterisk,
    Slash,

    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadDecimal,

    NumpadLeftArrow,
    NumpadRightArrow,
    NumpadUpArrow,
    NumpadDownArrow,

    NumpadPageUp,
    NumpadPageDown,
    NumpadEnd,
    NumpadHome,
    NumpadDelete,
    NumpadInsert,
    NumpadBegin,

    NumpadEnter,

    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,

    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    Numpad0,

    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}
#[derive(Debug, Clone)]
pub enum LogicalKey<Str = SmolStr> {
    Key(Key),
    Character(Str),
    /// OS symbol
    Unknown(u32),
}
impl LogicalKey<SmolStr> {
    pub fn as_ref(&self) -> LogicalKey<&str> {
        match self {
            LogicalKey::Key(key) => LogicalKey::Key(*key),
            LogicalKey::Character(str) => LogicalKey::Character(str.as_str()),
            LogicalKey::Unknown(unk) => LogicalKey::Unknown(*unk),
        }
    }
}

// bitflags::bitflags! {
//     #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
//     pub struct KeyModifiers: u8 {
//         const SHIFT = 1 << 0;
//         const CTRL = 1 << 1;
//         const ALT = 1 << 2;
//         const SUPER = 1 << 3;
//     }
// }
