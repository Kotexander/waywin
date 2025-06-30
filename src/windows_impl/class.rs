use super::utils::{instance, PWSTRING};
use std::num::NonZero;
use windows::{
    core::PCWSTR,
    Win32::UI::WindowsAndMessaging::{
        RegisterClassExW, UnregisterClassW, CS_HREDRAW, CS_VREDRAW, WNDCLASSEXW,
    },
};

pub struct WindowClass {
    name: PWSTRING,
    // _atom: NonZero<u16>,
}
impl WindowClass {
    pub fn new(name: &str) -> Result<Self, String> {
        let name = PWSTRING::from(name);

        let win_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | windows::Win32::UI::WindowsAndMessaging::CS_OWNDC,
            lpfnWndProc: Some(super::window::wndproc),
            hInstance: instance(),
            lpszClassName: name.as_pcwstr(),
            ..Default::default()
        };
        let _atom = NonZero::new(unsafe { RegisterClassExW(&win_class) })
            .ok_or_else(windows::core::Error::from_win32)
            .map_err(|err| format!("failed to register class: {err}"))?;

        Ok(Self { name })
    }

    pub fn name(&self) -> PCWSTR {
        self.name.as_pcwstr()
    }
}
impl Drop for WindowClass {
    fn drop(&mut self) {
        if let Err(err) = unsafe { UnregisterClassW(self.name.as_pcwstr(), Some(instance())) } {
            log::error!("Failed to unregister class '{}': {err}", unsafe {
                self.name.as_pcwstr().to_string().unwrap()
            },);
        }
    }
}
