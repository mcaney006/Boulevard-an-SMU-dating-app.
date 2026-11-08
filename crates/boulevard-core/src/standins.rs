//! The eight fictional stand-in students from the prototype. They let the
//! product work before a live campus. The Worker only serves them when
//! `STANDINS_ENABLED` is on, always labels them as scripted, and their
//! "likes" are scripted, not real swipes.

use serde::{Deserialize, Serialize};

use crate::types::{Gender, Intent, Year};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StandIn {
    /// Stable id, always prefixed `standin:`.
    pub id: String,
    pub name: String,
    pub gender: Gender,
    pub year: Year,
    pub major: String,
    pub school: String,
    pub greek: String,
    pub intent: Intent,
    pub bio: String,
    pub photo: String,
    pub likes_you: bool,
    pub note: String,
}

pub const STANDIN_PREFIX: &str = "standin:";

pub fn is_standin_id(id: &str) -> bool {
    id.starts_with(STANDIN_PREFIX)
}

#[allow(clippy::too_many_arguments)]
fn s(
    id: &str,
    name: &str,
    gender: Gender,
    year: Year,
    major: &str,
    school: &str,
    greek: &str,
    intent: Intent,
    bio: &str,
    likes_you: bool,
    note: &str,
) -> StandIn {
    StandIn {
        id: format!("{STANDIN_PREFIX}{id}"),
        name: name.into(),
        gender,
        year,
        major: major.into(),
        school: school.into(),
        greek: greek.into(),
        intent,
        bio: bio.into(),
        photo: format!("/people/{id}.jpg"),
        likes_you,
        note: note.into(),
    }
}

pub fn standins() -> Vec<StandIn> {
    vec![
        s("ava", "Ava Chen", Gender::Woman, Year::Junior, "Finance", "Cox", "Kappa Kappa Gamma", Intent::Dating,
          "Cox junior who will leave a formal early if the conversation is better than the band. Looking for someone who can be direct.",
          true, "Your major filter is close enough. If you're actually on campus this week, say so."),
        s("maya", "Maya Ellis", Gender::Woman, Year::Sophomore, "Film", "Meadows", "", Intent::Relationship,
          "Independent, Meadows, allergic to 'let's just see.' I want a real plan, not a 1 a.m. porch.",
          false, ""),
        s("elena", "Elena Vargas", Gender::Woman, Year::Sophomore, "Marketing", "Cox", "Pi Beta Phi", Intent::Casual,
          "Here for something light and specific. If your profile says relationship and your chats say otherwise, I'm gone.",
          true, "Intent match. Don't open with a compliment about the photo."),
        s("priya", "Priya Shah", Gender::Woman, Year::Junior, "Political Science", "Dedman", "", Intent::Dating,
          "Dedman, debate, and a low tolerance for vague plans. Coffee on the Hilltop beats another app that hides the ranking.",
          true, "I liked you because the intent line was dating, not a shrug."),
        s("camille", "Camille Brooks", Gender::Woman, Year::Senior, "Journalism", "Meadows", "Kappa Alpha Theta", Intent::Dating,
          "Senior, leaving in May, not interested in a semester of almost. Tell me what you're actually looking for.",
          false, ""),
        s("sofia", "Sofia Nguyen", Gender::Woman, Year::FirstYear, "Economics", "Dedman", "Delta Delta Delta", Intent::Casual,
          "First-year who already knows the group-chat version of everyone. Verified students only is the whole point.",
          false, ""),
        s("liam", "Liam Ortiz", Gender::Man, Year::Senior, "Mechanical Engineering", "Lyle", "Sigma Chi", Intent::Casual,
          "Lyle, Sigma Chi, done pretending the app is a personality test. Clear intent, then a real plan.",
          true, "If you're in Lyle or just nearby, that's enough to start."),
        s("ethan", "Ethan Park", Gender::Man, Year::Junior, "Computer Science", "Lyle", "", Intent::Relationship,
          "Independent, builds things, not interested in a deck that lies about why you showed up. Relationship or nothing.",
          false, ""),
    ]
}

pub fn standin_by_id(id: &str) -> Option<StandIn> {
    standins().into_iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{is_greek, is_major};

    #[test]
    fn eight_standins_with_catalog_values() {
        let all = standins();
        assert_eq!(all.len(), 8);
        for p in &all {
            assert!(is_major(&p.major), "{} major not in catalog", p.name);
            assert!(is_greek(&p.greek), "{} house not in catalog", p.name);
            assert!(is_standin_id(&p.id));
        }
        assert_eq!(all.iter().filter(|p| p.likes_you).count(), 4);
    }
}
