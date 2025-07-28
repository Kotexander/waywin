#[derive(Debug, Clone, Copy)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    Forward,
    Back,
    Unknown(u32),
}

#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Vertical,
    Horizontal,
}
impl ScrollDirection {
    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Vertical)
    }
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Horizontal)
    }
}
