//! The browser session an Acting Person works through (ADR-0006).

/// Identifies one browser session, e.g. to hold a Meeting Lock. Opaque to
/// core: it is minted and carried by the api layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
