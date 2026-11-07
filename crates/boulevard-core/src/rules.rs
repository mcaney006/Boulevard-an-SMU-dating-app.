//! The published ranking rules. The API serves these verbatim and the score in
//! `score.rs` is the executable version. Change both together and bump the
//! version.

pub const RULES_VERSION: &str = "2026-11-07";

pub const POINTS_SAME_INTENT: i32 = 40;
pub const POINTS_SAME_MAJOR: i32 = 25;
pub const POINTS_SAME_HOUSE: i32 = 20;
pub const POINTS_YEAR_WITHIN_ONE: i32 = 15;
pub const POINTS_ALREADY_LIKED_YOU: i32 = 15;

/// Free members get this many likes per Chicago calendar day.
pub const FREE_DAILY_LIKES: u32 = 8;

pub const PLUS_PRICE_LABEL: &str = "$12 / month";

pub const RULES: &[&str] = &[
    "Same intent: +40. This is a hard signal, not a suggestion.",
    "Same major: +25.",
    "Both in a house, same house: +20. One independent: no greek points.",
    "Year within one step: +15. Same year: +15. Further apart: +0.",
    "They already liked you: +15, shown before you swipe only on Boulevard+.",
    "Ties: whoever was active most recently comes first; still tied, the earlier member comes first.",
    "Nothing else moves the order. No hidden score, no boost slot, no paid reroll.",
];

pub fn published_rules() -> Vec<String> {
    RULES.iter().map(|r| r.to_string()).collect()
}
