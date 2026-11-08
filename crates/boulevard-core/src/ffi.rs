//! UniFFI surface for the iOS and Android apps. Keep it a thin, stable wrapper
//! over the pure functions so Swift and Kotlin never reimplement a rule.

use crate::catalog::{CATALOG_VERSION, GREEKS, MAJORS};
use crate::rules::{FREE_DAILY_LIKES, PLUS_PRICE_LABEL, RULES_VERSION};
use crate::score::{Score, ScoreCandidate, ScoreProfile};
use crate::standins::StandIn;
use crate::types::{Profile, ProfileInput};
use crate::validate::{EmailCheck, FieldError};

#[derive(Debug, Clone, uniffi::Record)]
pub struct Catalog {
    pub version: u32,
    pub majors: Vec<String>,
    pub greeks: Vec<String>,
    pub rules_version: String,
    pub rules: Vec<String>,
    pub free_daily_likes: u32,
    pub plus_price_label: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileValidation {
    pub profile: Option<Profile>,
    pub errors: Vec<FieldError>,
}

#[uniffi::export]
pub fn check_email(raw: String) -> EmailCheck {
    crate::validate::normalize_email(&raw)
}

#[uniffi::export]
pub fn score_candidate(me: ScoreProfile, them: ScoreCandidate, reveal_like: bool) -> Score {
    crate::score::score(&me, &them, reveal_like)
}

#[uniffi::export]
pub fn validate_profile_input(input: ProfileInput) -> ProfileValidation {
    match crate::validate::validate_profile(&input) {
        Ok(profile) => ProfileValidation {
            profile: Some(profile),
            errors: vec![],
        },
        Err(errors) => ProfileValidation {
            profile: None,
            errors,
        },
    }
}

#[uniffi::export]
pub fn catalog() -> Catalog {
    Catalog {
        version: CATALOG_VERSION,
        majors: MAJORS.iter().map(|m| m.to_string()).collect(),
        greeks: GREEKS.iter().map(|g| g.to_string()).collect(),
        rules_version: RULES_VERSION.to_string(),
        rules: crate::rules::published_rules(),
        free_daily_likes: FREE_DAILY_LIKES,
        plus_price_label: PLUS_PRICE_LABEL.to_string(),
    }
}

#[uniffi::export]
pub fn stand_ins() -> Vec<StandIn> {
    crate::standins::standins()
}

#[uniffi::export]
pub fn chicago_day_start_ms(now_ms: i64) -> i64 {
    crate::time::chicago_day_start_ms(now_ms)
}
