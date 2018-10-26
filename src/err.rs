use std::error;
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

pub fn to_io<E>() -> impl Fn(E) -> io::Error
where
    E: Into<Box<dyn error::Error + Send + Sync>>,
{
    |e| io::Error::new(io::ErrorKind::Other, e)
}
