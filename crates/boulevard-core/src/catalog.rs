//! Editable catalogs. These are the only lists the profile form offers.
//!
//! The first eight majors and the first eight houses are the ones the original
//! prototype shipped with (the stand-in deck depends on them). The rest are a
//! starting list of SMU programs and chapters; edit freely and bump
//! `CATALOG_VERSION` so clients can refresh.

pub const CATALOG_VERSION: u32 = 1;

pub const INDEPENDENT: &str = "Independent";

pub const MAJORS: &[&str] = &[
    "Finance",
    "Marketing",
    "Computer Science",
    "Journalism",
    "Political Science",
    "Film",
    "Mechanical Engineering",
    "Economics",
    "Accounting",
    "Management",
    "Real Estate",
    "Advertising",
    "Corporate Communication and Public Affairs",
    "Computer Engineering",
    "Electrical Engineering",
    "Civil Engineering",
    "Data Science",
    "Mathematics",
    "Statistics",
    "Biology",
    "Chemistry",
    "Physics",
    "Psychology",
    "Sociology",
    "Anthropology",
    "International Studies",
    "Public Policy",
    "Human Rights",
    "History",
    "English",
    "Philosophy",
    "Religious Studies",
    "Theatre",
    "Dance",
    "Music",
    "Art",
    "Art History",
    "Creative Computing",
    "Health and Society",
    "Applied Physiology and Sport Management",
    "Education",
    "Other",
];

/// Houses. `Independent` is a valid choice and scores no house points.
pub const GREEKS: &[&str] = &[
    INDEPENDENT,
    "Kappa Kappa Gamma",
    "Pi Beta Phi",
    "Delta Delta Delta",
    "Kappa Alpha Theta",
    "Sigma Chi",
    "Kappa Alpha",
    "Pi Kappa Alpha",
    "Alpha Chi Omega",
    "Chi Omega",
    "Delta Gamma",
    "Gamma Phi Beta",
    "Alpha Epsilon Pi",
    "Alpha Tau Omega",
    "Beta Theta Pi",
    "Beta Upsilon Chi",
    "Delta Sigma Phi",
    "Kappa Sigma",
    "Lambda Chi Alpha",
    "Phi Delta Theta",
    "Phi Gamma Delta",
    "Sigma Alpha Epsilon",
    "Sigma Phi Epsilon",
    "Theta Chi",
    "Other house",
];

pub fn is_major(s: &str) -> bool {
    MAJORS.iter().any(|m| *m == s)
}

pub fn is_greek(s: &str) -> bool {
    s.is_empty() || GREEKS.iter().any(|g| *g == s)
}

/// Normalise the house field: `Independent` and whitespace become empty.
pub fn normalize_greek(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() || t == INDEPENDENT {
        String::new()
    } else {
        t.to_string()
    }
}
