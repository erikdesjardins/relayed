use std::fmt::{self, Debug, Display};
use std::ops::RangeInclusive;

use failure::Error;

#[derive(Debug, Fail)]
#[fail(display = "optional was None")]
pub struct NoneError(());

pub trait OptionExt {
    type Out;
    fn into_result(self) -> Result<Self::Out, NoneError>;
}

impl<T> OptionExt for Option<T> {
    type Out = T;
    fn into_result(self) -> Result<Self::Out, NoneError> {
        match self {
            Some(x) => Ok(x),
            None => Err(NoneError(())),
        }
    }
}

pub struct ShowCauses(pub Error);

impl Debug for ShowCauses {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut causes = self.0.iter_chain();
        if let Some(error) = causes.next() {
            write!(f, "{}", error)?;
        }
        for error in causes {
            write!(f, "\n{}", error)?;
        }
        Ok(())
    }
}

impl Display for ShowCauses {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut causes = self.0.iter_chain();
        if let Some(error) = causes.next() {
            write!(f, "{}", error)?;
        }
        for error in causes {
            write!(f, ": {}", error)?;
        }
        Ok(())
    }
}

impl<E: Into<Error>> From<E> for ShowCauses {
    fn from(error: E) -> Self {
        ShowCauses(error.into())
    }
}

pub struct Backoff {
    value: u64,
    min: u64,
    max: u64,
}

impl Backoff {
    fn new(range: RangeInclusive<u64>) -> Self {
        Backoff {
            value: *range.start(),
            min: *range.start(),
            max: *range.end(),
        }
    }

    fn get(&mut self) -> u64 {
        let value = self.value;
        self.value = self.value.saturating_mul(2).max(self.max);
        value
    }

    fn reset(&mut self) {
        self.value = self.min;
    }
}
