use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicUsize, Ordering::*};

pub struct Backoff {
    value: AtomicUsize,
    min: usize,
    max: usize,
}

impl Backoff {
    pub fn new(range: RangeInclusive<usize>) -> Self {
        Backoff {
            value: (*range.start()).into(),
            min: *range.start(),
            max: *range.end(),
        }
    }

    pub fn get(&self) -> usize {
        loop {
            let old_value = self.value.load(Relaxed);
            let new_value = old_value.saturating_mul(2).min(self.max);
            match self
                .value
                .compare_exchange(old_value, new_value, SeqCst, Relaxed)
            {
                Ok(old_value) => return old_value,
                Err(_) => continue,
            }
        }
    }

    pub fn reset(&self) {
        self.value.store(self.min, SeqCst);
    }
}
