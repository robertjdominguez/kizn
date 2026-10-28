//! The pure domain: no IO, no async. Everything that counts is computed here
//! from rows the store loads. Time and randomness come in through the
//! [`Clock`] and [`Rng`] traits so scenarios can say "3 days later" and
//! shuffles are reproducible.

pub mod bank;
pub mod clock;
pub mod deck;
pub mod graph;
pub mod id;
pub mod lecture;
pub mod progress;
pub mod rng;
pub mod source;
pub mod tier;

pub use clock::{Clock, FakeClock, ShiftedClock, SystemClock, parse_sql_time, sql_time};
pub use id::new_id;
pub use rng::{Rng, SeededRng};
pub use tier::Tier;
