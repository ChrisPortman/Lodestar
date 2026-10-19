//! Project Codes: validation (FR-17) and suggestion from a name (FR-16).

use serde::{Deserialize, Serialize};

pub const MAX_CODE_CHARS: usize = 10;

/// Letters a suggestion is built from.
const SUGGESTION_LETTERS: usize = 4;

/// Words left out of suggestions, unless nothing else remains.
const FILLER_WORDS: &[&str] = &[
    "a", "an", "and", "at", "by", "for", "in", "of", "on", "or", "the", "to", "with",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectCodeError {
    #[error("project code must be an uppercase letter followed by 1–9 uppercase letters or digits")]
    Invalid,
}

/// A Project Code: an uppercase letter followed by 1–9 uppercase letters or digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProjectCode(String);

impl ProjectCode {
    pub fn new(raw: impl AsRef<str>) -> Result<Self, ProjectCodeError> {
        let raw = raw.as_ref();
        let mut chars = raw.chars();
        let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_uppercase());
        let rest_ok = chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        let len = raw.chars().count();
        if starts_with_letter && rest_ok && (2..=MAX_CODE_CHARS).contains(&len) {
            Ok(Self(raw.to_owned()))
        } else {
            Err(ProjectCodeError::Invalid)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ProjectCode {
    type Error = ProjectCodeError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ProjectCode> for String {
    fn from(value: ProjectCode) -> Self {
        value.0
    }
}

impl std::fmt::Display for ProjectCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Suggests a Project Code for `name` (FR-16), adding a number if `is_taken`
/// says the plain acronym is in use. `None` if the name has under two letters.
pub fn suggest_code(name: &str, is_taken: impl Fn(&str) -> bool) -> Option<ProjectCode> {
    let base = acronym(name);
    if base.len() < 2 {
        return None;
    }
    if !is_taken(&base) {
        return ProjectCode::new(base).ok();
    }
    (1u32..)
        .map(|n| format!("{base}{n}"))
        .take_while(|code| code.len() <= MAX_CODE_CHARS)
        .find(|code| !is_taken(code))
        .and_then(|code| ProjectCode::new(code).ok())
}

fn acronym(name: &str) -> String {
    let words: Vec<String> = name
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(char::is_ascii_alphabetic)
                .map(|c| c.to_ascii_uppercase())
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect();
    let meaningful: Vec<&String> = words
        .iter()
        .filter(|w| !FILLER_WORDS.contains(&w.to_ascii_lowercase().as_str()))
        .collect();
    let words: Vec<&String> = if meaningful.is_empty() {
        words.iter().collect()
    } else {
        meaningful
    };

    if words.len() >= SUGGESTION_LETTERS {
        return words
            .iter()
            .take(SUGGESTION_LETTERS)
            .filter_map(|w| w.chars().next())
            .collect();
    }
    // Share the letters out, earlier words taking the extras; a word that runs
    // out passes what it couldn't give to the next.
    let n = words.len();
    let mut code = String::new();
    let mut carry = 0;
    for (i, word) in words.iter().enumerate() {
        let quota = SUGGESTION_LETTERS / n + usize::from(i < SUGGESTION_LETTERS % n) + carry;
        let take = quota.min(word.len());
        code.push_str(&word[..take]);
        carry = quota - take;
    }
    code
}
