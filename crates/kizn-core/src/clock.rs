use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};

/// The only way code learns the time.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// A clock that stays where it's put: for scenarios and the dev bridge's
/// shifted clock.
pub struct FakeClock(Mutex<DateTime<Utc>>);

impl FakeClock {
    pub fn at(t: DateTime<Utc>) -> Self {
        Self(Mutex::new(t))
    }

    pub fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

/// UTC as SQLite's `datetime('now')` writes it, so stored times compare as
/// text.
pub fn sql_time(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn fake_clock_advances() {
        let c = FakeClock::at(Utc.with_ymd_and_hms(2026, 10, 25, 9, 0, 0).unwrap());
        c.advance(Duration::days(3));
        assert_eq!(sql_time(c.now()), "2026-10-28 09:00:00");
    }
}
