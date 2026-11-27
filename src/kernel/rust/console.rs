use crate::bindings;
use core::{fmt, marker::PhantomData};

/// Exclusive access to the VGA console during early boot.
pub(crate) struct BootConsole {
    _cpu_local: PhantomData<*mut ()>,
}

impl BootConsole {
    /// # Safety
    ///
    /// The VGA console and its cursor must be initialized and mapped.
    /// No other code may access the console while this handle is live.
    pub(crate) unsafe fn new() -> Self {
        Self {
            _cpu_local: PhantomData,
        }
    }

    pub(crate) fn write(&mut self, text: &str) {
        // SAFETY: The handle guarantees console access. kwrite reads exactly
        // text.len() bytes during this call and does not keep the pointer.
        unsafe { bindings::kwrite(text.as_ptr().cast(), text.len() as u64, 0x0f) };
    }
}

impl fmt::Write for BootConsole {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.write(text);
        Ok(())
    }
}
