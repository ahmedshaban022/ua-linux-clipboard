//! Time as a trait so the paste pipeline and policy logic are testable
//! with a fake clock.

pub trait Clock {
    /// Milliseconds since the Unix epoch.
    fn now_ms(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Test double: steps only when the test says so.
pub struct FakeClock(std::cell::Cell<u64>);

impl FakeClock {
    pub fn new(start_ms: u64) -> Self {
        FakeClock(std::cell::Cell::new(start_ms))
    }
    pub fn advance(&self, ms: u64) {
        self.0.set(self.0.get() + ms);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_advances() {
        let c = FakeClock::new(1000);
        assert_eq!(c.now_ms(), 1000);
        c.advance(50);
        assert_eq!(c.now_ms(), 1050);
    }

    #[test]
    fn system_clock_is_sane() {
        assert!(SystemClock.now_ms() > 1_600_000_000_000);
    }
}
