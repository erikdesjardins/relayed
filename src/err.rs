use std::fmt::{self, Debug, Display};
use std::io;

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

pub trait IoErrorExt {
    fn applies_to(&self) -> AppliesTo;
}

impl IoErrorExt for io::Error {
    fn applies_to(&self) -> AppliesTo {
        match self.kind() {
            io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset => AppliesTo::Connection,
            _ => AppliesTo::Listener,
        }
    }
}

pub enum AppliesTo {
    Connection,
    Listener,
}
