//! Talking to models: the two wire protocols kizn speaks (OpenAI-compatible
//! chat completions, Anthropic Messages), the connection test, and errors in
//! words a learner can act on. No domain logic: the model writes and talks;
//! code elsewhere keeps score.
//!
//! The `fake` feature adds a local server that speaks both protocols with
//! scripted behaviours, for tests and the dev bridge's mock mode. Nothing
//! that ships enables it.

mod client;
mod error;
mod probe;
mod sse;

#[cfg(feature = "fake")]
pub mod fake;

pub use client::{Client, Endpoint, Http, JsonMode, Protocol, REDIRECT_HOST_HEADER, Usage};
pub use error::ModelError;
pub use probe::{Check, Structured, probe};
pub use reqwest;
