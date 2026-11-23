use serde::{Deserialize, Serialize};

/// What someone says they are here for. The strongest signal in the score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Intent {
    Casual,
    Dating,
    Relationship,
}

impl Intent {
    pub const ALL: [Intent; 3] = [Intent::Casual, Intent::Dating, Intent::Relationship];

    pub fn as_str(self) -> &'static str {
        match self {
            Intent::Casual => "casual",
            Intent::Dating => "dating",
            Intent::Relationship => "relationship",
        }
    }

    /// The wording shown on cards.
    pub fn label(self) -> &'static str {
        match self {
            Intent::Casual => "Something casual",
            Intent::Dating => "Actually dating",
            Intent::Relationship => "A relationship",
        }
    }

    pub fn parse(s: &str) -> Option<Intent> {
        Intent::ALL.into_iter().find(|i| i.as_str() == s)
    }
}

/// Class year. Order matters: "within one step" is index distance <= 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Year {
    FirstYear,
    Sophomore,
    Junior,
    Senior,
    Graduate,
}

impl Year {
    pub const ALL: [Year; 5] = [
        Year::FirstYear,
        Year::Sophomore,
        Year::Junior,
        Year::Senior,
        Year::Graduate,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Year::FirstYear => "first-year",
            Year::Sophomore => "sophomore",
            Year::Junior => "junior",
            Year::Senior => "senior",
            Year::Graduate => "graduate",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Year::FirstYear => "First-year",
            Year::Sophomore => "Sophomore",
            Year::Junior => "Junior",
            Year::Senior => "Senior",
            Year::Graduate => "Graduate",
        }
    }

    pub fn index(self) -> i32 {
        Year::ALL.iter().position(|y| *y == self).unwrap_or(0) as i32
    }

    pub fn parse(s: &str) -> Option<Year> {
        Year::ALL
            .into_iter()
            .find(|y| y.as_str() == s || y.label().eq_ignore_ascii_case(s))
    }
}

/// Who a member wants to see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Looking {
    Women,
    Men,
    Everyone,
}

impl Looking {
    pub const ALL: [Looking; 3] = [Looking::Women, Looking::Men, Looking::Everyone];

    pub fn as_str(self) -> &'static str {
        match self {
            Looking::Women => "women",
            Looking::Men => "men",
            Looking::Everyone => "everyone",
        }
    }

    pub fn parse(s: &str) -> Option<Looking> {
        Looking::ALL.into_iter().find(|l| l.as_str() == s)
    }

    /// Whether a member with this preference wants to see someone of `gender`.
    pub fn accepts(self, gender: Gender) -> bool {
        matches!(
            (self, gender),
            (Looking::Everyone, _) | (Looking::Women, Gender::Woman) | (Looking::Men, Gender::Man)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Gender {
    Woman,
    Man,
    Nonbinary,
}

impl Gender {
    pub const ALL: [Gender; 3] = [Gender::Woman, Gender::Man, Gender::Nonbinary];

    pub fn as_str(self) -> &'static str {
        match self {
            Gender::Woman => "woman",
            Gender::Man => "man",
            Gender::Nonbinary => "nonbinary",
        }
    }

    pub fn parse(s: &str) -> Option<Gender> {
        Gender::ALL.into_iter().find(|g| g.as_str() == s)
    }
}

/// A complete member profile as stored and as returned by the API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Profile {
    pub display_name: String,
    pub gender: Gender,
    pub year: Year,
    pub major: String,
    /// Greek house, or empty for independent. "Independent" normalises to empty.
    pub greek: String,
    pub intent: Intent,
    pub looking_for: Looking,
    pub bio: String,
}

/// The unvalidated shape a client submits. Strings everywhere so the validator
/// can return one error list instead of failing on the first bad enum.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ProfileInput {
    pub display_name: String,
    pub gender: String,
    pub year: String,
    pub major: String,
    pub greek: String,
    pub intent: String,
    pub looking_for: String,
    pub bio: String,
}
