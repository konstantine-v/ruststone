//! Validated identifiers and query values.
//!
//! Every value that ends up in a Lodestone URL goes through one of these
//! types, so malformed input is rejected with a 400 by the extractor, before
//! any upstream request is made, and cannot inject path segments.

use std::{fmt, num::NonZeroU16};

use serde::{de, Deserialize, Deserializer, Serialize};

macro_rules! numeric_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

numeric_id!(
    /// Lodestone character ID.
    CharacterId
);
numeric_id!(
    /// Lodestone free company ID (these exceed `u32`).
    FreeCompanyId
);
numeric_id!(
    /// Lodestone (world) linkshell ID.
    LinkshellId
);

macro_rules! hash_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl TryFrom<String> for $name {
            type Error = &'static str;

            fn try_from(mut s: String) -> Result<Self, Self::Error> {
                if s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
                    s.make_ascii_lowercase();
                    Ok(Self(s))
                } else {
                    Err(concat!(stringify!($name), " must be 40 hexadecimal characters"))
                }
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

hash_id!(
    /// Cross-world linkshell ID (40 hex characters).
    CwlsId
);
hash_id!(
    /// PvP team ID (40 hex characters).
    PvpTeamId
);

/// 1-based page number for paginated Lodestone lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageNo(pub NonZeroU16);

impl PageNo {
    pub const FIRST: Self = Self(NonZeroU16::MIN);

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

impl Default for PageNo {
    fn default() -> Self {
        Self::FIRST
    }
}

impl fmt::Display for PageNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Free-text search term: 1–64 characters, no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SearchName(String);

impl TryFrom<String> for SearchName {
    type Error = &'static str;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let trimmed = s.trim();
        let len = trimmed.chars().count();
        if !(1..=64).contains(&len) {
            return Err("name must be 1 to 64 characters");
        }
        if trimmed.chars().any(char::is_control) {
            return Err("name must not contain control characters");
        }
        Ok(Self(trimmed.to_owned()))
    }
}

impl From<SearchName> for String {
    fn from(v: SearchName) -> Self {
        v.0
    }
}

impl AsRef<str> for SearchName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// World or data center name: 2–20 ASCII letters (e.g. `Gilgamesh`, `Aether`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorldName(String);

impl TryFrom<String> for WorldName {
    type Error = &'static str;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if (2..=20).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphabetic()) {
            Ok(Self(s))
        } else {
            Err("world/dc must be 2 to 20 ASCII letters")
        }
    }
}

impl From<WorldName> for String {
    fn from(v: WorldName) -> Self {
        v.0
    }
}

impl AsRef<str> for WorldName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Comma-separated list in a query string (`?include=a,b`), each item parsed
/// with `T`'s own `Deserialize` (so enums keep their serde names). Duplicates
/// are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommaList<T>(pub Vec<T>);

impl<T> Default for CommaList<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: PartialEq> CommaList<T> {
    pub fn contains(&self, item: &T) -> bool {
        self.0.contains(item)
    }
}

impl<'de, T> Deserialize<'de> for CommaList<T>
where
    T: Deserialize<'de> + PartialEq,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        let mut items = Vec::new();
        for part in raw.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let item = T::deserialize(de::value::StrDeserializer::<de::value::Error>::new(part))
                .map_err(|e| de::Error::custom(format!("`{part}`: {e}")))?;
            if !items.contains(&item) {
                items.push(item);
            }
        }
        Ok(Self(items))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_ids_are_validated_and_lowercased() {
        let ok = CwlsId::try_from("ABCDEF0123456789abcdef0123456789ABCDEF01".to_owned()).unwrap();
        assert_eq!(ok.to_string(), "abcdef0123456789abcdef0123456789abcdef01");
        assert!(CwlsId::try_from("../../etc".to_owned()).is_err());
        assert!(PvpTeamId::try_from("a".repeat(39)).is_err());
    }

    #[test]
    fn search_values_are_validated() {
        assert!(SearchName::try_from("  ".to_owned()).is_err());
        assert!(SearchName::try_from("x".repeat(65)).is_err());
        assert_eq!(
            SearchName::try_from(" Y'shtola Rhul ".to_owned())
                .unwrap()
                .as_ref(),
            "Y'shtola Rhul"
        );
        assert!(WorldName::try_from("Gilgamesh".to_owned()).is_ok());
        assert!(WorldName::try_from("Gil&x=1".to_owned()).is_err());
    }

    #[test]
    fn page_zero_is_rejected() {
        assert!(serde_json::from_str::<PageNo>("0").is_err());
        assert_eq!(serde_json::from_str::<PageNo>("3").unwrap().get(), 3);
    }

    #[test]
    fn comma_list_parses_enums_and_dedupes() {
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum S {
            ClassJobs,
            Mounts,
        }
        #[derive(Deserialize)]
        struct Q {
            include: CommaList<S>,
        }
        let q: Q = serde_json::from_str(r#"{"include":"mounts, class_jobs,mounts"}"#).unwrap();
        assert_eq!(q.include.0, vec![S::Mounts, S::ClassJobs]);
        assert!(serde_json::from_str::<Q>(r#"{"include":"nope"}"#).is_err());
    }
}
