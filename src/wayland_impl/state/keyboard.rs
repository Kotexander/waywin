use super::WaywinState;
use crate::event::{Event, Key, PhysicalKey, Text, WindowEvent};
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
                    // state.
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
                    state.events.push(WindowEvent {
                        kind: Event::Focus(false),
                        window_id: focused_window,
                    });
                }

                // focus new window
                let id = surface.id().as_ptr() as usize;
                state.keyboard.focused_window = Some(id);
                state.events.push(WindowEvent {
                    kind: Event::Focus(true),
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
                    state.events.push(WindowEvent {
                        kind: Event::Focus(false),
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
                let key = xkb::Keycode::new(key + 8);

                if let Some(token) = state.keyboard.repeat_state.take() {
                    state.handle.remove(token.token);
                }

                let Some(id) = state.keyboard.focused_window else {
                    log::warn!("recieved a key down event while no window is focused");
                    return;
                };

                if let Some(xkb_state) = &state.keyboard.xkb_state {
                    let kind = Event::Key {
                        down: true,
                        physical_key: PhysicalKey::from(key),
                        text: Text::from(xkb_state.key_get_utf8(key)),
                    };

                    state.events.push(WindowEvent {
                        kind: kind.clone(),
                        window_id: id,
                    });

                    if xkb_state.get_keymap().key_repeats(key) {
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
                                        state.events.push(WindowEvent {
                                            kind: kind.clone(),
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
                let key = xkb::Keycode::new(key + 8);

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
                    let kind = Event::Key {
                        down: false,
                        physical_key: PhysicalKey::from(key),
                        text: Text::from(xkb_state.key_get_utf8(key)),
                    };

                    state.events.push(WindowEvent {
                        kind: kind.clone(),
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
        PhysicalKey::Key(match value.raw() {
            23 => Key::Tab,
            113 => Key::LeftArrow,
            114 => Key::RightArrow,
            111 => Key::UpArrow,
            116 => Key::DownArrow,
            112 => Key::PageUp,
            117 => Key::PageDown,
            110 => Key::Home,
            115 => Key::End,
            118 => Key::Insert,
            119 => Key::Delete,
            22 => Key::Backspace,
            65 => Key::Space,
            36 => Key::Enter,
            9 => Key::Escape,
            37 => Key::LCtrl,
            50 => Key::LShift,
            64 => Key::LAlt,
            133 => Key::LSuper,
            105 => Key::RCtrl,
            62 => Key::RShift,
            107 => Key::RAlt,
            134 => Key::RSuper,
            135 => Key::Menu,
            10 => Key::Key1,
            11 => Key::Key2,
            12 => Key::Key3,
            13 => Key::Key4,
            14 => Key::Key5,
            15 => Key::Key6,
            16 => Key::Key7,
            17 => Key::Key8,
            18 => Key::Key9,
            19 => Key::Key0,
            87 => Key::Numpad1,
            88 => Key::Numpad2,
            89 => Key::Numpad3,
            83 => Key::Numpad4,
            84 => Key::Numpad5,
            85 => Key::Numpad6,
            79 => Key::Numpad7,
            80 => Key::Numpad8,
            81 => Key::Numpad9,
            90 => Key::Numpad0,
            91 => Key::NumpadDecimal,
            106 => Key::NumpadDivide,
            63 => Key::NumpadMultiply,
            82 => Key::NumpadSubtract,
            86 => Key::NumpadAdd,
            104 => Key::NumpadEnter,
            38 => Key::A,
            56 => Key::B,
            54 => Key::C,
            40 => Key::D,
            26 => Key::E,
            41 => Key::F,
            42 => Key::G,
            43 => Key::H,
            31 => Key::I,
            44 => Key::J,
            45 => Key::K,
            46 => Key::L,
            58 => Key::M,
            57 => Key::N,
            32 => Key::O,
            33 => Key::P,
            24 => Key::Q,
            27 => Key::R,
            39 => Key::S,
            28 => Key::T,
            30 => Key::U,
            55 => Key::V,
            25 => Key::W,
            53 => Key::X,
            29 => Key::Y,
            52 => Key::Z,
            67 => Key::F1,
            68 => Key::F2,
            69 => Key::F3,
            70 => Key::F4,
            71 => Key::F5,
            72 => Key::F6,
            73 => Key::F7,
            74 => Key::F8,
            75 => Key::F9,
            76 => Key::F10,
            95 => Key::F11,
            96 => Key::F12,

            47 => Key::Semicolon,
            48 => Key::Quote,

            59 => Key::Comma,
            60 => Key::Period,
            61 => Key::Slash,

            20 => Key::Minus,
            21 => Key::Equal,

            34 => Key::LBracket,
            35 => Key::RBracket,
            51 => Key::Backslash,

            49 => Key::Grave,
            66 => Key::CapsLock,
            78 => Key::ScrollLock,
            77 => Key::NumLock,

            108 => Key::PrintScreen,
            127 => Key::Pause,

            _ => return PhysicalKey::Unknown(value.raw()),
        })
    }
}
