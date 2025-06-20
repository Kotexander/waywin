use windows::Win32::{Foundation::HINSTANCE, System::SystemServices::IMAGE_DOS_HEADER};

pub fn loword(l: usize) -> usize {
    l & 0xffff
}
pub fn hiword(h: usize) -> usize {
    (h >> 16) & 0xffff
}

pub fn instance() -> HINSTANCE {
    extern "C" {
        static __ImageBase: IMAGE_DOS_HEADER;
    }

    HINSTANCE(unsafe { &__ImageBase as *const _ as _ })
}
