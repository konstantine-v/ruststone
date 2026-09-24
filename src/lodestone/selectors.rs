//! The vendored lodestone-css-selectors files (`assets/selectors/`), embedded
//! at compile time and compiled into typed selector groups at boot.

use serde::de::DeserializeOwned;

use super::pages::{
    achievements::AchievementsSels,
    character::CharacterSels,
    class_jobs::ClassJobSels,
    collections::CollectionSels,
    free_company::{FreeCompanySels, MembersSels},
    groups::{CwlsSels, GroupMembersSels, LinkshellSels, PvpTeamMembersSels, PvpTeamSels},
    search::SearchSels,
};

#[derive(Debug)]
pub struct Selectors {
    pub character: CharacterSels,
    pub class_jobs: ClassJobSels,
    pub achievements: AchievementsSels,
    pub mounts: CollectionSels,
    pub minions: CollectionSels,
    pub free_company: FreeCompanySels,
    pub free_company_members: MembersSels,
    pub linkshell: LinkshellSels,
    pub linkshell_members: GroupMembersSels,
    pub cwls: CwlsSels,
    pub cwls_members: GroupMembersSels,
    pub pvp_team: PvpTeamSels,
    pub pvp_team_members: PvpTeamMembersSels,
    pub search: SearchSels,
}

/// Embeds `assets/selectors/<path>` and parses it, naming the file on error.
macro_rules! file {
    ($path:literal) => {
        parse(
            $path,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/selectors/",
                $path
            )),
        )?
    };
}

impl Selectors {
    /// Compiles every CSS selector and regex in the vendored files.
    ///
    /// # Errors
    /// When a file is missing a field the parsers need, or contains an
    /// invalid selector or regex.
    pub fn load() -> loco_rs::Result<Self> {
        Ok(Self {
            character: CharacterSels {
                profile: file!("profile/character.json"),
                attributes: file!("profile/attributes.json"),
                gear: file!("profile/gearset.json"),
            },
            class_jobs: file!("profile/classjob.json"),
            achievements: file!("profile/achievements.json"),
            mounts: file!("profile/mount.json"),
            minions: file!("profile/minion.json"),
            free_company: FreeCompanySels {
                profile: file!("freecompany/freecompany.json"),
                focus: file!("freecompany/focus.json"),
                seeking: file!("freecompany/seeking.json"),
                reputation: file!("freecompany/reputation.json"),
            },
            free_company_members: file!("freecompany/members.json"),
            linkshell: file!("linkshell/ls.json"),
            linkshell_members: file!("linkshell/members.json"),
            cwls: file!("cwls/cwls.json"),
            cwls_members: file!("cwls/members.json"),
            pvp_team: file!("pvpteam/pvpteam.json"),
            pvp_team_members: file!("pvpteam/members.json"),
            search: SearchSels {
                character: file!("search/character.json"),
                free_company: file!("search/freecompany.json"),
                linkshell: file!("search/linkshell.json"),
                cwls: file!("search/cwls.json"),
                pvp_team: file!("search/pvpteam.json"),
            },
        })
    }
}

fn parse<T: DeserializeOwned>(path: &str, json: &str) -> loco_rs::Result<T> {
    serde_json::from_str(json)
        .map_err(|e| loco_rs::Error::Message(format!("selectors `{path}`: {e}")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn vendored_selectors_load() {
        if let Err(e) = super::Selectors::load() {
            panic!("{e}");
        }
    }
}
