use std::io;

pub enum Never {}

impl From<Never> for io::Error {
    fn from(never: Never) -> Self {
        match never {}
    }
}
