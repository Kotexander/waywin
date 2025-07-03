use crate::event::WindowEvent;
use class::WindowClass;
use std::{cell::UnsafeCell, rc::Rc};
pub use window::Window;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PostQuitMessage, TranslateMessage, MSG,
};

mod class;
mod utils;
mod window;

type EventHook = Rc<UnsafeCell<Option<Box<dyn FnMut(WindowEvent)>>>>;

pub struct Waywin {
    /// All created windows keep a pointer to this so **do not move it**
    event_hook: EventHook,
    window_class: Rc<WindowClass>,
}
impl Waywin {
    pub fn init(class_name: &str) -> std::result::Result<Self, String> {
        let window_class = Rc::new(WindowClass::new(class_name)?);

        let event_hook = Rc::new(UnsafeCell::new(None));

        Ok(Self {
            event_hook,
            window_class,
        })
    }
    pub fn exit(&self) {
        unsafe { PostQuitMessage(0) }
    }
    pub fn run(&self, event_hook: impl FnMut(WindowEvent) + 'static) {
        // TODO: this is still unsafe and a really bad way of doing things

        unsafe { assert!((*self.event_hook.get()).is_none()) }

        unsafe {
            *self.event_hook.get() = Some(Box::new(event_hook));
        }

        // // erasing the the lifetime of the event hook.
        // // Safety: i think its ok since the event hook gets unset
        // // at the end of the function perserving liftimes...
        // // and as long as waywin doesn't do anything else funny.
        // // unsafe {
        // //     *self.event_hook.get() = Some(Box::new(std::mem::transmute::<
        // //         Box<dyn FnMut(WindowEvent)>,
        // //         Box<dyn FnMut(WindowEvent)>,
        // //     >(Box::new(event_hook))));
        // // }

        let mut message = MSG::default();

        unsafe {
            while GetMessageW(std::ptr::addr_of_mut!(message), None, 0, 0).as_bool() {
                let _ = TranslateMessage(std::ptr::addr_of_mut!(message));
                DispatchMessageW(std::ptr::addr_of!(message));
            }
        }

        // this is important to keep lifetimes
        unsafe {
            *self.event_hook.get() = None;
        }
    }
}

impl std::fmt::Debug for Waywin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waywin").finish_non_exhaustive()
    }
}
