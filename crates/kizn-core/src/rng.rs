use std::sync::Mutex;

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// The only source of randomness: ids, option shuffles.
pub trait Rng: Send + Sync {
    fn next_u128(&self) -> u128;
}

/// Seeded from the OS in the app, from a fixed seed in scenarios.
pub struct SeededRng(Mutex<StdRng>);

impl SeededRng {
    pub fn from_seed(seed: u64) -> Self {
        Self(Mutex::new(StdRng::seed_from_u64(seed)))
    }

    pub fn from_os() -> Self {
        Self(Mutex::new(rand::make_rng()))
    }
}

impl Rng for SeededRng {
    fn next_u128(&self) -> u128 {
        self.0.lock().unwrap().random()
    }
}
