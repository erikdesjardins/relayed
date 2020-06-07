use std::ops::RangeInclusive;

pub struct Backoff {
    value: u8,
    min: u8,
    max: u8,
}

impl Backoff {
    pub fn new(range: RangeInclusive<u8>) -> Self {
        Backoff {
            value: *range.start(),
            min: *range.start(),
            max: *range.end(),
        }
    }

    pub fn next(&mut self) -> u8 {
        let old_value = self.value;
        self.value = old_value.saturating_mul(2).min(self.max);
        old_value
    }

    pub fn reset(&mut self) {
        self.value = self.min;
    }
}
