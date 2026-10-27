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

/// The system clock, some days ahead: the dev bridge's `KIZN_DAYS_LATER`
/// and the test bridge's `/test/later`, so a learner's week can be walked in
/// one sitting.
#[derive(Default)]
pub struct ShiftedClock(std::sync::atomic::AtomicI64);

impl ShiftedClock {
    pub fn days(days: i64) -> Self {
        Self(std::sync::atomic::AtomicI64::new(days))
    }

    /// Moves the clock this many days further on.
    pub fn later(&self, days: i64) {
        self.0.fetch_add(days, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn reset(&self) {
        self.0.store(0, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Clock for ShiftedClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now() + Duration::days(self.0.load(std::sync::atomic::Ordering::SeqCst))
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

/// Reads a time written by [`sql_time`].
pub fn parse_sql_time(s: &str) -> Option<DateTime<Utc>> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok().map(|t| t.and_utc())
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
