use windows::core::PCWSTR;

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
