use super::WaywinState;
use crate::event::{Key, KeyCode, LogicalKey, PhysicalKey, WaywinEvent, WindowEvent};
use smol_str::SmolStr;
use std::time::Duration;
use wayland_client::{
    protocol::wl_keyboard::{self, KeyState, KeymapFormat, WlKeyboard},
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
};
use xkbcommon::xkb;

#[derive(Debug, Clone, Copy)]
pub struct RepeatInfo {
    pub delay: Duration,
    pub repeat: Duration,
}

pub struct RepeatState {
    pub token: calloop::RegistrationToken,
    pub key: xkb::Keycode,
}

pub struct KeyboardState {
    pub keyboard: Option<WlKeyboard>,
    pub repeat_info: Option<RepeatInfo>,
    pub repeat_state: Option<RepeatState>,
    pub focused_window: Option<usize>,
    pub xkb_context: xkb::Context,
    pub xkb_state: Option<xkb::State>,
}

fn keysym_to_utf8_smol(keysym: xkb::Keysym) -> SmolStr {
    use std::ffi::*;
    unsafe {
        let buf: &mut [c_char] = &mut [0; 8];
        let ptr = &mut buf[0] as *mut c_char;
        match xkb::ffi::xkb_keysym_to_utf8(keysym.raw(), ptr, 8) {
            0 => SmolStr::new_static(""),
            -1 => {
                panic!("Key doesn't fit in buffer")
            }
            len => {
                let slice: &[u8] = std::slice::from_raw_parts(ptr as *const _, len as usize - 1);
                SmolStr::new_inline(std::str::from_utf8_unchecked(slice))
            }
        }
    }
}
fn xkb_state_key_get_utf8_smol(xkb_state: &xkb::State, key: xkb::Keycode) -> SmolStr {
    use std::ffi::*;
    unsafe {
        const BUF_LEN: usize = 64;
        let buf: &mut [c_char] = &mut [0; BUF_LEN];
        let ptr = &mut buf[0] as *mut c_char;
        let ret =
            xkb::ffi::xkb_state_key_get_utf8(xkb_state.get_raw_ptr(), key.into(), ptr, BUF_LEN);
        let len = ret.max(0).min(BUF_LEN as i32);
        let slice: &[u8] = std::slice::from_raw_parts(ptr as *const _, len as usize);
        SmolStr::new_inline(std::str::from_utf8_unchecked(slice))
    }
}

fn generate_down_event(
    xkb_state: &xkb::State,
    wayland_key: xkb::Keycode,
    key: xkb::Keycode,
) -> WindowEvent {
    let layout = xkb_state.key_get_layout(wayland_key);
    let keysym = xkb_state.key_get_one_sym(wayland_key);
    let unmodified_keysym = xkb_state
        .get_keymap()
        .key_get_syms_by_level(wayland_key, layout, 0)[0];

    let physical_key = PhysicalKey::from(key);
    let logical_key = LogicalKey::from(keysym);
    let logical_key_unmodified = LogicalKey::from(unmodified_keysym);

    let text = match &logical_key {
        LogicalKey::Key(_) | LogicalKey::Unknown(_) => keysym_to_utf8_smol(keysym),
        LogicalKey::Character(c) => c.clone(),
    };
    let text_raw = xkb_state_key_get_utf8_smol(xkb_state, wayland_key);

    WindowEvent::Key {
        down: true,
        physical_key,
        text,
        logical_key,
        text_raw,
        logical_key_unmodified,
    }
}

fn generate_up_event(
    xkb_state: &xkb::State,
    wayland_key: xkb::Keycode,
    key: xkb::Keycode,
) -> WindowEvent {
    let layout = xkb_state.key_get_layout(wayland_key);
    let keysym = xkb_state.key_get_one_sym(wayland_key);
    let unmodified_keysym = xkb_state
        .get_keymap()
        .key_get_syms_by_level(wayland_key, layout, 0)[0];

    let physical_key = PhysicalKey::from(key);
    let logical_key = LogicalKey::from(keysym);
    let logical_key_unmodified = LogicalKey::from(unmodified_keysym);

    WindowEvent::Key {
        down: false,
        physical_key,
        text: SmolStr::new_static(""),
        logical_key,
        text_raw: SmolStr::new_static(""),
        logical_key_unmodified,
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            keyboard: None,
            repeat_info: None,
            repeat_state: None,
            focused_window: None,
            xkb_context: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            xkb_state: None,
        }
    }
}

impl Dispatch<WlKeyboard, ()> for WaywinState {
    fn event(
        state: &mut Self,
        _proxy: &WlKeyboard,
        event: <WlKeyboard as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log::debug!("{event:?}");
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                state.keyboard.xkb_state = None;
                if let WEnum::Value(KeymapFormat::XkbV1) = format {
                    let keymap = unsafe {
                        xkb::Keymap::new_from_fd(
                            &state.keyboard.xkb_context,
                            fd,
                            size as usize,
                            xkb::KEYMAP_FORMAT_TEXT_V1,
                            xkb::KEYMAP_COMPILE_NO_FLAGS,
                        )
                        .unwrap()
                        .unwrap()
                    };
                    let xkb_state = xkb::State::new(&keymap);
                    state.keyboard.xkb_state = Some(xkb_state);
                } else {
                    log::warn!("unkown keymap")
                }
            }
            wl_keyboard::Event::Enter {
                serial: _,
                surface,
                keys: _, // TODO
            } => {
                // unfocus old window if it wasn't already
                if let Some(focused_window) = state.keyboard.focused_window {
                    log::warn!("focusing new window before unfocusing previous window");
                    state.events.push(WaywinEvent::WindowEvent {
                        event: WindowEvent::Focus(false),
                        window_id: focused_window,
                    });
                }

                // focus new window
                let id = surface.id().as_ptr() as usize;
                state.keyboard.focused_window = Some(id);
                state.events.push(WaywinEvent::WindowEvent {
                    event: WindowEvent::Focus(true),
                    window_id: id,
                });
            }
            wl_keyboard::Event::Leave { serial: _, surface } => {
                if let Some(token) = state.keyboard.repeat_state.take() {
                    state.handle.remove(token.token);
                }
                let id = surface.id().as_ptr() as usize;
                if Some(id) != state.keyboard.focused_window {
                    log::warn!("unfocusing an unfocused window: {id}");
                } else {
                    state.keyboard.focused_window = None;
                    state.events.push(WaywinEvent::WindowEvent {
                        event: WindowEvent::Focus(false),
                        window_id: id,
                    });
                }
            }
            wl_keyboard::Event::Key {
                serial: _,
                time: _,
                key,
                state: WEnum::Value(KeyState::Pressed),
            } => {
                let wayland_key = xkb::Keycode::new(key + 8);
                let key = xkb::Keycode::new(key);

                if let Some(token) = state.keyboard.repeat_state.take() {
                    state.handle.remove(token.token);
                }

                let Some(id) = state.keyboard.focused_window else {
                    log::warn!("recieved a key down event while no window is focused");
                    return;
                };

                if let Some(xkb_state) = &state.keyboard.xkb_state {
                    let event = generate_down_event(xkb_state, wayland_key, key);

                    state.events.push(WaywinEvent::WindowEvent {
                        event: event.clone(),
                        window_id: id,
                    });

                    if xkb_state.get_keymap().key_repeats(wayland_key) {
                        if let Some(repeat_info) = &state.keyboard.repeat_info {
                            let timer = calloop::timer::Timer::from_duration(repeat_info.delay);
                            let token = state
                                .handle
                                .insert_source(timer, move |_, _, state| {
                                    let Some(id) = state.keyboard.focused_window else {
                                        log::warn!(
                                            "tried a key repeat event while no window is focused"
                                        );
                                        return calloop::timer::TimeoutAction::Drop;
                                    };

                                    if let Some(repeat_info) = state.keyboard.repeat_info {
                                        state.events.push(WaywinEvent::WindowEvent {
                                            event: event.clone(),
                                            window_id: id,
                                        });

                                        calloop::timer::TimeoutAction::ToDuration(
                                            repeat_info.repeat,
                                        )
                                    } else {
                                        calloop::timer::TimeoutAction::Drop
                                    }
                                })
                                .unwrap();
                            state.keyboard.repeat_state = Some(RepeatState { token, key });
                        }
                    }
                }
            }
            wl_keyboard::Event::Key {
                serial: _,
                time: _,
                key,
                state: WEnum::Value(KeyState::Released),
            } => {
                let wayland_key = xkb::Keycode::new(key + 8);
                let key = xkb::Keycode::new(key);

                let Some(id) = state.keyboard.focused_window else {
                    log::warn!("recieved a key up event while no window is focused");
                    return;
                };

                // remove repeat callback if keycode is the same
                if let Some(repeat_state) = state
                    .keyboard
                    .repeat_state
                    .take_if(|token| token.key == key)
                {
                    state.handle.remove(repeat_state.token);
                }

                if let Some(xkb_state) = &state.keyboard.xkb_state {
                    let kind = generate_up_event(xkb_state, wayland_key, key);

                    state.events.push(WaywinEvent::WindowEvent {
                        event: kind.clone(),
                        window_id: id,
                    });
                }
            }
            wl_keyboard::Event::Key {
                serial: _,
                time: _,
                key: _,
                state: WEnum::Unknown(_),
            } => {
                log::error!("unknown key state sent by OS")
            }
            wl_keyboard::Event::Modifiers {
                serial: _,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => {
                if let Some(xkb_state) = &mut state.keyboard.xkb_state {
                    xkb_state.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);

                    // let Some(id) = state.keyboard.focused_window else {
                    //     log::warn!("recieved key modifiers event while no window is focused");
                    //     return;
                    // };

                    // let key_modifiers = if xkb_state
                    //     .mod_name_is_active(xkb::MOD_NAME_SHIFT, xkb::STATE_MODS_EFFECTIVE)
                    // {
                    //     KeyModifiers::SHIFT
                    // } else {
                    //     KeyModifiers::empty()
                    // } | if xkb_state
                    //     .mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE)
                    // {
                    //     KeyModifiers::CTRL
                    // } else {
                    //     KeyModifiers::empty()
                    // } | if xkb_state
                    //     .mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE)
                    // {
                    //     KeyModifiers::ALT
                    // } else {
                    //     KeyModifiers::empty()
                    // };

                    // state.events.push(WindowEvent {
                    //     kind: Event::KeyModifiers(key_modifiers),
                    //     window_id: id,
                    // });
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                if rate == 0 {
                    state.keyboard.repeat_info = None;
                    if let Some(repeat_state) = state.keyboard.repeat_state.take() {
                        state.handle.remove(repeat_state.token);
                    }
                } else {
                    state.keyboard.repeat_info = Some(RepeatInfo {
                        delay: Duration::from_millis(delay as u64),
                        repeat: Duration::from_millis(1000 / rate as u64),
                    });
                }
            }
            _ => unimplemented!(),
        }
    }
}

impl From<xkb::Keycode> for PhysicalKey {
    fn from(value: xkb::Keycode) -> Self {
        Self::KeyCode(match value.raw() {
            15 => KeyCode::Tab,
            105 => KeyCode::LeftArrow,
            106 => KeyCode::RightArrow,
            103 => KeyCode::UpArrow,
            108 => KeyCode::DownArrow,
            104 => KeyCode::PageUp,
            109 => KeyCode::PageDown,
            102 => KeyCode::Home,
            107 => KeyCode::End,
            110 => KeyCode::Insert,
            111 => KeyCode::Delete,
            14 => KeyCode::Backspace,
            57 => KeyCode::Space,
            28 => KeyCode::Enter,
            1 => KeyCode::Escape,
            29 => KeyCode::LCtrl,
            42 => KeyCode::LShift,
            56 => KeyCode::LAlt,
            125 => KeyCode::LSuper,
            97 => KeyCode::RCtrl,
            54 => KeyCode::RShift,
            99 => KeyCode::RAlt,
            126 => KeyCode::RSuper,
            127 => KeyCode::Menu,
            2 => KeyCode::Key1,
            3 => KeyCode::Key2,
            4 => KeyCode::Key3,
            5 => KeyCode::Key4,
            6 => KeyCode::Key5,
            7 => KeyCode::Key6,
            8 => KeyCode::Key7,
            9 => KeyCode::Key8,
            10 => KeyCode::Key9,
            11 => KeyCode::Key0,
            79 => KeyCode::Numpad1,
            80 => KeyCode::Numpad2,
            81 => KeyCode::Numpad3,
            75 => KeyCode::Numpad4,
            76 => KeyCode::Numpad5,
            77 => KeyCode::Numpad6,
            71 => KeyCode::Numpad7,
            72 => KeyCode::Numpad8,
            73 => KeyCode::Numpad9,
            82 => KeyCode::Numpad0,
            83 => KeyCode::NumpadDecimal,
            98 => KeyCode::NumpadDivide,
            55 => KeyCode::NumpadMultiply,
            74 => KeyCode::NumpadSubtract,
            78 => KeyCode::NumpadAdd,
            96 => KeyCode::NumpadEnter,
            30 => KeyCode::A,
            48 => KeyCode::B,
            46 => KeyCode::C,
            32 => KeyCode::D,
            18 => KeyCode::E,
            33 => KeyCode::F,
            34 => KeyCode::G,
            35 => KeyCode::H,
            23 => KeyCode::I,
            36 => KeyCode::J,
            37 => KeyCode::K,
            38 => KeyCode::L,
            50 => KeyCode::M,
            49 => KeyCode::N,
            24 => KeyCode::O,
            25 => KeyCode::P,
            16 => KeyCode::Q,
            19 => KeyCode::R,
            31 => KeyCode::S,
            20 => KeyCode::T,
            22 => KeyCode::U,
            47 => KeyCode::V,
            17 => KeyCode::W,
            45 => KeyCode::X,
            21 => KeyCode::Y,
            44 => KeyCode::Z,
            59 => KeyCode::F1,
            60 => KeyCode::F2,
            61 => KeyCode::F3,
            62 => KeyCode::F4,
            63 => KeyCode::F5,
            64 => KeyCode::F6,
            65 => KeyCode::F7,
            66 => KeyCode::F8,
            67 => KeyCode::F9,
            68 => KeyCode::F10,
            87 => KeyCode::F11,
            88 => KeyCode::F12,

            39 => KeyCode::Semicolon,
            40 => KeyCode::Quote,

            51 => KeyCode::Comma,
            52 => KeyCode::Period,
            53 => KeyCode::Slash,

            12 => KeyCode::Minus,
            13 => KeyCode::Equal,

            26 => KeyCode::LBracket,
            27 => KeyCode::RBracket,
            43 => KeyCode::Backslash,

            41 => KeyCode::Grave,
            58 => KeyCode::CapsLock,
            70 => KeyCode::ScrollLock,
            69 => KeyCode::NumLock,

            100 => KeyCode::PrintScreen,
            119 => KeyCode::Pause,

            _ => return Self::Unknown(value.raw()),
        })
    }
}
impl From<xkb::Keysym> for LogicalKey {
    fn from(value: xkb::Keysym) -> Self {
        Self::Key(match value {
            xkb::Keysym::Tab => Key::Tab,
            xkb::Keysym::space => Key::Space,
            xkb::Keysym::period => Key::Period,

            xkb::Keysym::BackSpace => Key::Backspace,
            xkb::Keysym::Return => Key::Enter,
            xkb::Keysym::Escape => Key::Escape,
            xkb::Keysym::Control_L => Key::LCtrl,
            xkb::Keysym::Shift_L => Key::LShift,
            xkb::Keysym::Alt_L => Key::LAlt,
            xkb::Keysym::Super_L => Key::LSuper,
            xkb::Keysym::Control_R => Key::RCtrl,
            xkb::Keysym::Shift_R => Key::RShift,
            xkb::Keysym::Alt_R => Key::RAlt,
            xkb::Keysym::Super_R => Key::RSuper,

            xkb::Keysym::Left => Key::LeftArrow,
            xkb::Keysym::Right => Key::RightArrow,
            xkb::Keysym::Up => Key::UpArrow,
            xkb::Keysym::Down => Key::DownArrow,
            xkb::Keysym::Page_Up => Key::PageUp,
            xkb::Keysym::Page_Down => Key::PageDown,
            xkb::Keysym::Home => Key::Home,
            xkb::Keysym::End => Key::End,
            xkb::Keysym::Insert => Key::Insert,
            xkb::Keysym::Delete => Key::Delete,

            xkb::Keysym::plus => Key::Plus,
            xkb::Keysym::minus => Key::Minus,
            xkb::Keysym::asterisk => Key::Asterisk,
            xkb::Keysym::slash => Key::Slash,

            xkb::Keysym::KP_Add => Key::NumpadAdd,
            xkb::Keysym::KP_Subtract => Key::NumpadSubtract,
            xkb::Keysym::KP_Multiply => Key::NumpadMultiply,
            xkb::Keysym::KP_Divide => Key::NumpadDivide,
            xkb::Keysym::KP_Decimal => Key::NumpadDecimal,

            xkb::Keysym::KP_Left => Key::NumpadLeftArrow,
            xkb::Keysym::KP_Right => Key::NumpadRightArrow,
            xkb::Keysym::KP_Up => Key::NumpadUpArrow,
            xkb::Keysym::KP_Down => Key::NumpadDownArrow,
            xkb::Keysym::KP_Page_Up => Key::NumpadPageUp,
            xkb::Keysym::KP_Page_Down => Key::NumpadPageDown,
            xkb::Keysym::KP_Home => Key::NumpadHome,
            xkb::Keysym::KP_End => Key::NumpadEnd,
            xkb::Keysym::KP_Insert => Key::NumpadInsert,
            xkb::Keysym::KP_Delete => Key::NumpadDelete,
            xkb::Keysym::KP_Begin => Key::NumpadBegin,

            xkb::Keysym::_1 => Key::Key1,
            xkb::Keysym::_2 => Key::Key2,
            xkb::Keysym::_3 => Key::Key3,
            xkb::Keysym::_4 => Key::Key4,
            xkb::Keysym::_5 => Key::Key5,
            xkb::Keysym::_6 => Key::Key6,
            xkb::Keysym::_7 => Key::Key7,
            xkb::Keysym::_8 => Key::Key8,
            xkb::Keysym::_9 => Key::Key9,
            xkb::Keysym::_0 => Key::Key0,

            xkb::Keysym::KP_1 => Key::Numpad1,
            xkb::Keysym::KP_2 => Key::Numpad2,
            xkb::Keysym::KP_3 => Key::Numpad3,
            xkb::Keysym::KP_4 => Key::Numpad4,
            xkb::Keysym::KP_5 => Key::Numpad5,
            xkb::Keysym::KP_6 => Key::Numpad6,
            xkb::Keysym::KP_7 => Key::Numpad7,
            xkb::Keysym::KP_8 => Key::Numpad8,
            xkb::Keysym::KP_9 => Key::Numpad9,
            xkb::Keysym::KP_0 => Key::Numpad0,

            xkb::Keysym::F1 => Key::F1,
            xkb::Keysym::F2 => Key::F2,
            xkb::Keysym::F3 => Key::F3,
            xkb::Keysym::F4 => Key::F4,
            xkb::Keysym::F5 => Key::F5,
            xkb::Keysym::F6 => Key::F6,
            xkb::Keysym::F7 => Key::F7,
            xkb::Keysym::F8 => Key::F8,
            xkb::Keysym::F9 => Key::F9,
            xkb::Keysym::F10 => Key::F10,
            xkb::Keysym::F11 => Key::F11,
            xkb::Keysym::F12 => Key::F12,

            xkb::Keysym::KP_Enter => Key::NumpadEnter,

            xkb::Keysym::Caps_Lock => Key::CapsLock,
            xkb::Keysym::Scroll_Lock => Key::ScrollLock,
            xkb::Keysym::Num_Lock => Key::NumLock,

            xkb::Keysym::Print => Key::PrintScreen,
            xkb::Keysym::Pause => Key::Pause,
            xkb::Keysym::Menu => Key::Menu,

            _ => {
                let character = keysym_to_utf8_smol(value);
                if character.is_empty() {
                    return Self::Unknown(value.raw());
                } else {
                    return Self::Character(character);
                }
            }
        })
    }
}
