use windows::{
    core::PCWSTR,
    Win32::{Foundation::HINSTANCE, System::SystemServices::IMAGE_DOS_HEADER},
};

pub fn loword(l: usize) -> u32 {
    (l & 0xffff) as u32
}
pub fn hiword(h: usize) -> u32 {
    ((h >> 16) & 0xffff) as u32
}
pub fn get_x(l: usize) -> i16 {
    loword(l) as i16
}
pub fn get_y(h: usize) -> i16 {
    hiword(h) as i16
}

pub fn instance() -> HINSTANCE {
    extern "C" {
        static __ImageBase: IMAGE_DOS_HEADER;
    }

    HINSTANCE(unsafe { &__ImageBase as *const _ as _ })
}

#[allow(clippy::upper_case_acronyms)]
pub struct PWSTRING(Vec<u16>);
impl From<&str> for PWSTRING {
    fn from(value: &str) -> Self {
        Self(value.encode_utf16().chain(std::iter::once(0)).collect())
    }
}
impl PWSTRING {
    pub const fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR::from_raw(self.0.as_ptr())
    }
}
