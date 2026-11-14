//! Read a memory size the way Docker's `--memory` does, as bytes or with a binary suffix.

use serde::Deserialize;

use crate::declarations::models::MemorySize;

impl MemorySize {
    /// `"512m"`, `"2g"` or `"1073741824"`; the size must be at least one byte.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim().to_ascii_lowercase();
        let (digits, scale) = match text.strip_suffix('b').unwrap_or(&text) {
            value if value.ends_with('k') => (&value[..value.len() - 1], 1u64 << 10),
            value if value.ends_with('m') => (&value[..value.len() - 1], 1u64 << 20),
            value if value.ends_with('g') => (&value[..value.len() - 1], 1u64 << 30),
            value => (value, 1),
        };
        digits
            .parse::<u64>()
            .ok()
            .and_then(|count| count.checked_mul(scale))
            .filter(|bytes| *bytes > 0)
            .map(Self)
            .ok_or_else(|| {
                format!("memory must be a positive size such as \"512m\" or \"2g\", not {text:?}")
            })
    }
}

impl<'de> Deserialize<'de> for MemorySize {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Written {
            Bytes(u64),
            Text(String),
        }
        match Written::deserialize(deserializer)? {
            Written::Bytes(0) => Err(serde::de::Error::custom("memory must be at least one byte")),
            Written::Bytes(bytes) => Ok(Self(bytes)),
            Written::Text(text) => Self::parse(&text).map_err(serde::de::Error::custom),
        }
    }
}
