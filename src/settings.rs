//! Typed application settings, read from the `settings:` block of
//! `config/{env}.yaml`. Unknown keys are rejected so typos fail at boot.

use std::time::Duration;

use serde::Deserialize;
use url::Url;

use crate::lodestone::PageKind;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub lodestone: LodestoneSettings,
    pub cache_ttl_secs: CacheTtl,
}

impl Settings {
    /// Parse from Loco's untyped `config.settings` (a missing block means all
    /// defaults).
    ///
    /// # Errors
    /// When the block has unknown keys or values of the wrong type.
    pub fn from_config(raw: Option<&serde_json::Value>) -> loco_rs::Result<Self> {
        raw.map_or_else(
            || Ok(Self::default()),
            |value| {
                Self::deserialize(value)
                    .map_err(|e| loco_rs::Error::Message(format!("invalid `settings`: {e}")))
            },
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LodestoneSettings {
    pub region: Region,
    /// Overrides `https://{region}.finalfantasyxiv.com/lodestone/` (tests, proxies).
    pub base_url: Option<Url>,
    pub timeout_ms: u64,
    /// Upper bound on simultaneous upstream requests.
    pub max_concurrent: usize,
    pub user_agent_desktop: String,
    /// Lodestone only renders mount/minion lists for mobile clients.
    pub user_agent_mobile: String,
}

impl Default for LodestoneSettings {
    fn default() -> Self {
        // User agents mirror the vendored `assets/selectors/meta.json`.
        Self {
            region: Region::Na,
            base_url: None,
            timeout_ms: 10_000,
            max_concurrent: 16,
            user_agent_desktop: "curl/7.73.0".into(),
            user_agent_mobile: "Mozilla/5.0 (iPhone; CPU OS 10_15_5 like Mac OS X) \
                                AppleWebKit/605.1.15 (KHTML, like Gecko) Version/12.1.1 \
                                Mobile/14E304 Safari/605.1.15"
                .into(),
        }
    }
}

impl LodestoneSettings {
    /// The Lodestone root every page path is joined onto (always ends in `/`).
    ///
    /// # Errors
    /// When `base_url` cannot be a base (e.g. `data:` URLs).
    pub fn base_url(&self) -> loco_rs::Result<Url> {
        let mut url = match &self.base_url {
            Some(url) => url.clone(),
            None => Url::parse(&format!(
                "https://{}.finalfantasyxiv.com/lodestone/",
                self.region.subdomain()
            ))
            .map_err(|e| loco_rs::Error::Message(e.to_string()))?,
        };
        if url.cannot_be_a_base() {
            return Err(loco_rs::Error::Message(format!(
                "lodestone base_url `{url}` cannot be a base URL"
            )));
        }
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        Ok(url)
    }

    #[must_use]
    pub const fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}

/// Lodestone regional site. Determines both the host and the page language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    #[default]
    Na,
    Eu,
    Fr,
    De,
    Jp,
}

impl Region {
    #[must_use]
    pub const fn subdomain(self) -> &'static str {
        match self {
            Self::Na => "na",
            Self::Eu => "eu",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Jp => "jp",
        }
    }
}

/// Cache lifetime per page kind, in seconds. `0` disables caching for a kind.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheTtl {
    pub character: u64,
    pub class_jobs: u64,
    pub achievements: u64,
    pub mounts: u64,
    pub minions: u64,
    pub free_company: u64,
    pub members: u64,
    pub linkshell: u64,
    pub pvp_team: u64,
    pub search: u64,
}

impl Default for CacheTtl {
    fn default() -> Self {
        Self {
            character: 1800,
            class_jobs: 1800,
            achievements: 3600,
            mounts: 3600,
            minions: 3600,
            free_company: 3600,
            members: 3600,
            linkshell: 3600,
            pvp_team: 3600,
            search: 300,
        }
    }
}

impl CacheTtl {
    #[must_use]
    pub const fn for_kind(&self, kind: PageKind) -> Duration {
        Duration::from_secs(match kind {
            PageKind::Character => self.character,
            PageKind::ClassJobs => self.class_jobs,
            PageKind::Achievements => self.achievements,
            PageKind::Mounts => self.mounts,
            PageKind::Minions => self.minions,
            PageKind::FreeCompany => self.free_company,
            PageKind::FreeCompanyMembers => self.members,
            PageKind::Linkshell | PageKind::CrossworldLinkshell => self.linkshell,
            PageKind::PvpTeam => self.pvp_team,
            PageKind::Search(_) => self.search,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_block_missing() {
        let s = Settings::from_config(None).unwrap();
        assert_eq!(
            s.lodestone.base_url().unwrap().as_str(),
            "https://na.finalfantasyxiv.com/lodestone/"
        );
    }

    #[test]
    fn rejects_unknown_keys() {
        let raw = serde_json::json!({ "lodestone": { "regoin": "eu" } });
        assert!(Settings::from_config(Some(&raw)).is_err());
    }

    #[test]
    fn base_url_gets_trailing_slash() {
        let raw = serde_json::json!({ "lodestone": { "base_url": "http://127.0.0.1:1/x" } });
        let s = Settings::from_config(Some(&raw)).unwrap();
        assert_eq!(
            s.lodestone.base_url().unwrap().as_str(),
            "http://127.0.0.1:1/x/"
        );
    }
}
