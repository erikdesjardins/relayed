use std::fmt::{self, Debug, Display};

pub struct DebugFromDisplay<T: Display>(T);

impl<T: Display> Debug for DebugFromDisplay<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<T: Display> From<T> for DebugFromDisplay<T> {
    fn from(display: T) -> Self {
        DebugFromDisplay(display)
    }
}
