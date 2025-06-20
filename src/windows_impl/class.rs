use super::{pwstring::PWSTRING, utils::instance};
use std::num::NonZero;
use windows::{
    core::PCWSTR,
    Win32::UI::WindowsAndMessaging::{RegisterClassExW, CS_HREDRAW, CS_VREDRAW, WNDCLASSEXW},
};

pub struct WindowClass {
    name: PWSTRING,
    _atom: NonZero<u16>,
}
impl WindowClass {
    pub fn new(name: &str) -> Result<Self, String> {
        let name = PWSTRING::from(name);

        let win_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(super::window::wndproc),
            hInstance: instance(),
            lpszClassName: name.as_pcwstr(),
            ..Default::default()
        };
        let atom = NonZero::new(unsafe { RegisterClassExW(&win_class) })
            .ok_or_else(windows::core::Error::from_win32)
            .map_err(|err| format!("register class: {err}"))?;

        Ok(Self { name, _atom: atom })
    }

    pub fn name(&self) -> PCWSTR {
        self.name.as_pcwstr()
    }
}
// impl Drop for WindowClass {
//     fn drop(&mut self) {
//         if let Err(err) = unregister_class(self.name.as_ptr()) {
//             log::error!(
//                 "Failed to unregister class '{}': {}",
//                 from_wide_str(&self.name),
//                 err
//             );
//         }
//         if let Err(_) = delete_object(self.background) {
//             log::error!("Failed to deleted background brush");
//         }
//     }
// }
