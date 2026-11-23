//! Input validation shared by the Worker and the apps.

use serde::{Deserialize, Serialize};

use crate::catalog::{is_greek, is_major, normalize_greek};
use crate::types::{Gender, Intent, Looking, Profile, ProfileInput, Year};

/// The only domain Boulevard accepts. SMU issues `@smu.edu` as the primary address.
pub const ALLOWED_DOMAIN: &str = "smu.edu";

pub const MAX_DISPLAY_NAME: usize = 40;
pub const MAX_BIO: usize = 280;
pub const MAX_NOTE: usize = 200;
pub const MAX_MESSAGE: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EmailCheck {
    pub ok: bool,
    /// Lower-cased, trimmed address when `ok`; empty otherwise.
    pub normalized: String,
    /// Human-readable reason when not `ok`.
    pub reason: String,
}

/// Normalise and gate an address. Rejects anything outside `@smu.edu`, plus
/// sub-addressing (`name+tag@`) so one inbox cannot mint several accounts.
pub fn normalize_email(raw: &str) -> EmailCheck {
    let s = raw.trim().to_ascii_lowercase();
    let fail = |reason: &str| EmailCheck {
        ok: false,
        normalized: String::new(),
        reason: reason.to_string(),
    };
    if s.is_empty() || s.len() > 254 {
        return fail("Enter your SMU email.");
    }
    let Some((local, domain)) = s.split_once('@') else {
        return fail("Enter your SMU email.");
    };
    if domain != ALLOWED_DOMAIN {
        return fail("Boulevard is for @smu.edu addresses only.");
    }
    if local.is_empty() || local.len() > 64 {
        return fail("That address does not look right.");
    }
    if local.contains('+') {
        return fail("Use your plain @smu.edu address, without a + tag.");
    }
    if local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        return fail("That address does not look right.");
    }
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return fail("That address does not look right.");
    }
    EmailCheck {
        ok: true,
        normalized: s,
        reason: String::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

fn err(field: &str, message: &str) -> FieldError {
    FieldError {
        field: field.to_string(),
        message: message.to_string(),
    }
}

/// Validate a submitted profile. Returns the normalised profile or every
/// problem at once so a form can show them all.
pub fn validate_profile(input: &ProfileInput) -> Result<Profile, Vec<FieldError>> {
    let mut errors = Vec::new();

    let display_name = input.display_name.trim().to_string();
    if display_name.is_empty() {
        errors.push(err("display_name", "Add a name."));
    } else if display_name.chars().count() > MAX_DISPLAY_NAME {
        errors.push(err("display_name", "Keep your name under 40 characters."));
    }

    let gender = Gender::parse(input.gender.trim());
    if gender.is_none() {
        errors.push(err("gender", "Pick one of woman, man, or nonbinary."));
    }
    let year = Year::parse(input.year.trim());
    if year.is_none() {
        errors.push(err("year", "Pick your year."));
    }
    let major = input.major.trim().to_string();
    if !is_major(&major) {
        errors.push(err("major", "Pick a major from the list."));
    }
    let greek_raw = input.greek.trim();
    if !is_greek(greek_raw) {
        errors.push(err("greek", "Pick a house from the list, or Independent."));
    }
    let intent = Intent::parse(input.intent.trim());
    if intent.is_none() {
        errors.push(err("intent", "Say what you actually want."));
    }
    let looking_for = Looking::parse(input.looking_for.trim());
    if looking_for.is_none() {
        errors.push(err("looking_for", "Pick who you want to see."));
    }
    let bio = input.bio.trim().to_string();
    if bio.is_empty() {
        errors.push(err("bio", "Write at least one line."));
    } else if bio.chars().count() > MAX_BIO {
        errors.push(err("bio", "Keep your bio under 280 characters."));
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Profile {
        display_name,
        gender: gender.unwrap(),
        year: year.unwrap(),
        major,
        greek: normalize_greek(greek_raw),
        intent: intent.unwrap(),
        looking_for: looking_for.unwrap(),
        bio,
    })
}

/// Trim and bound a like note. `None` when empty.
pub fn clean_note(raw: Option<&str>) -> Result<Option<String>, FieldError> {
    let Some(raw) = raw else { return Ok(None) };
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if t.chars().count() > MAX_NOTE {
        return Err(err("note", "Keep the note under 200 characters."));
    }
    Ok(Some(t.to_string()))
}

/// Trim and bound a chat message body.
pub fn clean_message(raw: &str) -> Result<String, FieldError> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(err("body", "Say something."));
    }
    if t.chars().count() > MAX_MESSAGE {
        return Err(err("body", "Keep it under 500 characters."));
    }
    Ok(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_smu_and_normalises_case() {
        let c = normalize_email("  Peruna@SMU.edu ");
        assert!(c.ok);
        assert_eq!(c.normalized, "peruna@smu.edu");
    }

    #[test]
    fn rejects_other_domains_and_subaddressing() {
        assert!(!normalize_email("peruna@gmail.com").ok);
        assert!(!normalize_email("peruna@mail.smu.edu").ok);
        assert!(!normalize_email("peruna+two@smu.edu").ok);
        assert!(!normalize_email("@smu.edu").ok);
        assert!(!normalize_email("per..una@smu.edu").ok);
        assert!(!normalize_email("peruna").ok);
    }

    fn good() -> ProfileInput {
        ProfileInput {
            display_name: "Peruna".into(),
            gender: "woman".into(),
            year: "junior".into(),
            major: "Finance".into(),
            greek: "Independent".into(),
            intent: "dating".into(),
            looking_for: "men".into(),
            bio: "Hilltop coffee beats a hidden ranking.".into(),
        }
    }

    #[test]
    fn valid_profile_normalises_independent_to_empty() {
        let p = validate_profile(&good()).unwrap();
        assert_eq!(p.greek, "");
        assert_eq!(p.year, Year::Junior);
    }

    #[test]
    fn collects_every_error() {
        let mut bad = good();
        bad.display_name = "   ".into();
        bad.major = "Underwater Basket Weaving".into();
        bad.bio = "x".repeat(281);
        let errs = validate_profile(&bad).unwrap_err();
        let fields: Vec<&str> = errs.iter().map(|e| e.field.as_str()).collect();
        assert_eq!(fields, ["display_name", "major", "bio"]);
    }

    #[test]
    fn year_accepts_label_or_slug() {
        let mut p = good();
        p.year = "First-year".into();
        assert_eq!(validate_profile(&p).unwrap().year, Year::FirstYear);
    }

    #[test]
    fn notes_and_messages_are_bounded() {
        assert_eq!(clean_note(Some("  ")).unwrap(), None);
        assert_eq!(clean_note(Some(" hi ")).unwrap(), Some("hi".into()));
        assert!(clean_note(Some(&"n".repeat(201))).is_err());
        assert!(clean_message("").is_err());
        assert_eq!(clean_message(" ok ").unwrap(), "ok");
    }
}
