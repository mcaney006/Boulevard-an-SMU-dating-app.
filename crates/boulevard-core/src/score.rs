//! The published score, ported line for line from the prototype's `rank.ts`.

use serde::{Deserialize, Serialize};

use crate::catalog::normalize_greek;
use crate::rules::*;
use crate::types::{Intent, Year};

/// The fields of the viewer's profile that the score reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ScoreProfile {
    pub intent: Intent,
    pub major: String,
    pub greek: String,
    pub year: Year,
}

/// The fields of a candidate that the score reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ScoreCandidate {
    pub intent: Intent,
    pub major: String,
    pub greek: String,
    pub year: Year,
    /// They already liked the viewer. Only counted when `reveal_like` is true.
    pub likes_you: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Reason {
    pub label: String,
    pub points: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Score {
    pub total: i32,
    pub reasons: Vec<Reason>,
}

fn reason(label: &str, points: i32) -> Reason {
    Reason {
        label: label.to_string(),
        points,
    }
}

/// Score `them` for `me`. `reveal_like` is true only for Boulevard+ members;
/// free members never see the "already liked you" points before swiping.
pub fn score(me: &ScoreProfile, them: &ScoreCandidate, reveal_like: bool) -> Score {
    let mut reasons = Vec::with_capacity(5);

    if me.intent == them.intent {
        reasons.push(reason("Same intent", POINTS_SAME_INTENT));
    }
    if me.major == them.major {
        reasons.push(reason("Same major", POINTS_SAME_MAJOR));
    }
    let my_house = normalize_greek(&me.greek);
    let their_house = normalize_greek(&them.greek);
    if !my_house.is_empty() && !their_house.is_empty() && my_house == their_house {
        reasons.push(reason("Same house", POINTS_SAME_HOUSE));
    }
    let a = me.year.index();
    let b = them.year.index();
    if (a - b).abs() <= 1 {
        let label = if a == b { "Same year" } else { "Adjacent year" };
        reasons.push(reason(label, POINTS_YEAR_WITHIN_ONE));
    }
    if them.likes_you && reveal_like {
        reasons.push(reason("Already liked you", POINTS_ALREADY_LIKED_YOU));
    }

    let total = reasons.iter().map(|r| r.points).sum();
    Score { total, reasons }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me() -> ScoreProfile {
        ScoreProfile {
            intent: Intent::Dating,
            major: "Finance".into(),
            greek: "Kappa Kappa Gamma".into(),
            year: Year::Junior,
        }
    }

    fn them(intent: Intent, major: &str, greek: &str, year: Year, likes_you: bool) -> ScoreCandidate {
        ScoreCandidate {
            intent,
            major: major.into(),
            greek: greek.into(),
            year,
            likes_you,
        }
    }

    #[test]
    fn full_match_without_reveal_is_100() {
        let s = score(&me(), &them(Intent::Dating, "Finance", "Kappa Kappa Gamma", Year::Junior, true), false);
        assert_eq!(s.total, 100);
        let labels: Vec<&str> = s.reasons.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Same intent", "Same major", "Same house", "Same year"]);
    }

    #[test]
    fn reveal_adds_fifteen_only_on_plus() {
        let c = them(Intent::Dating, "Finance", "Kappa Kappa Gamma", Year::Junior, true);
        assert_eq!(score(&me(), &c, true).total, 115);
        assert_eq!(score(&me(), &c, false).total, 100);
        let not_liked = them(Intent::Dating, "Finance", "Kappa Kappa Gamma", Year::Junior, false);
        assert_eq!(score(&me(), &not_liked, true).total, 100);
    }

    #[test]
    fn independent_never_scores_house_points() {
        let mut viewer = me();
        viewer.greek = "Independent".into();
        let c = them(Intent::Casual, "Film", "Independent", Year::FirstYear, false);
        assert_eq!(score(&viewer, &c, false).total, 0);
        viewer.greek = String::new();
        let c2 = them(Intent::Casual, "Film", "", Year::FirstYear, false);
        assert_eq!(score(&viewer, &c2, false).total, 0);
    }

    #[test]
    fn adjacent_year_counts_two_apart_does_not() {
        let adj = them(Intent::Casual, "Film", "", Year::Senior, false);
        let s = score(&me(), &adj, false);
        assert_eq!(s.total, 15);
        assert_eq!(s.reasons[0].label, "Adjacent year");
        let far = them(Intent::Casual, "Film", "", Year::FirstYear, false);
        assert_eq!(score(&me(), &far, false).total, 0);
    }

    #[test]
    fn graduate_is_adjacent_to_senior_only() {
        let mut viewer = me();
        viewer.year = Year::Graduate;
        assert_eq!(score(&viewer, &them(Intent::Casual, "Film", "", Year::Senior, false), false).total, 15);
        assert_eq!(score(&viewer, &them(Intent::Casual, "Film", "", Year::Junior, false), false).total, 0);
    }
}
