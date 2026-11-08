//! Boulevard domain core.
//!
//! Everything here is pure Rust with no I/O so it compiles for the Cloudflare
//! Worker (`wasm32-unknown-unknown`) and for iOS/Android through UniFFI. The
//! Worker and the apps call the same code for the published score, profile
//! validation, and the `@smu.edu` gate, so the rules live in exactly one place.

pub mod catalog;
pub mod rules;
pub mod score;
pub mod standins;
pub mod time;
pub mod types;
pub mod validate;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use rules::{published_rules, RULES_VERSION};
pub use score::{score, Reason, Score, ScoreCandidate, ScoreProfile};
pub use types::{Gender, Intent, Looking, Profile, Year};
pub use validate::{normalize_email, validate_profile, EmailCheck, FieldError};

#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();
