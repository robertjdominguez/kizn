use ulid::Ulid;

use crate::{Clock, Rng};

/// A ULID from the injected clock and randomness, so ids sort by creation
/// time and are reproducible under a fake clock and a fixed seed.
pub fn new_id(clock: &dyn Clock, rng: &dyn Rng) -> String {
    let ms = clock.now().timestamp_millis().max(0) as u64;
    Ulid::from_parts(ms, rng.next_u128()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FakeClock, SeededRng};
    use chrono::{TimeZone, Utc};

    #[test]
    fn ids_are_reproducible_and_sort_by_time() {
        let clock = FakeClock::at(Utc.with_ymd_and_hms(2026, 10, 25, 9, 0, 0).unwrap());
        let a = new_id(&clock, &SeededRng::from_seed(7));
        assert_eq!(a, new_id(&clock, &SeededRng::from_seed(7)));
        clock.advance(chrono::Duration::seconds(1));
        assert!(new_id(&clock, &SeededRng::from_seed(1)) > a);
    }
}
