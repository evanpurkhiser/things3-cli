use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentifierToken(String);

impl IdentifierToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl FromStr for IdentifierToken {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value.is_empty() {
            return Err("identifier cannot be empty".to_string());
        }
        Ok(Self(value.to_string()))
    }
}

impl From<String> for IdentifierToken {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for IdentifierToken {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl fmt::Display for IdentifierToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
