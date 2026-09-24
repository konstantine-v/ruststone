//! `character/{id}/`: profile, attributes and gear, all from one page
//! (`profile/character.json` + `attributes.json` + `gearset.json`).

use std::collections::BTreeMap;

use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};
use url::Url;

use super::common::{Crest, CrestSels, IconName, IconNameSels, World};
use crate::lodestone::{
    ids::{CharacterId, FreeCompanyId, PvpTeamId},
    page_url,
    selector::Sel,
    Page, PageKind, ParseError, Selectors,
};

#[derive(Debug, Serialize)]
pub struct Character {
    pub id: CharacterId,
    pub name: String,
    pub title: Option<String>,
    pub world: Option<World>,
    pub race: Option<String>,
    pub tribe: Option<String>,
    pub gender: Option<Gender>,
    pub nameday: Option<String>,
    pub guardian_deity: Option<IconName>,
    pub city_state: Option<IconName>,
    pub grand_company: Option<GrandCompanyRank>,
    pub free_company: Option<FreeCompanyRef>,
    pub pvp_team: Option<PvpTeamRef>,
    pub bio: Option<String>,
    pub avatar: Option<Url>,
    pub portrait: Option<Url>,
    pub active_class_job: ActiveClassJob,
    pub attributes: Attributes,
    pub gear: BTreeMap<GearSlot, GearPiece>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Male,
    Female,
}

impl Gender {
    fn from_symbol(s: &str) -> Option<Self> {
        match s {
            "♂" => Some(Self::Male),
            "♀" => Some(Self::Female),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GrandCompanyRank {
    pub name: String,
    pub rank: String,
}

#[derive(Debug, Serialize)]
pub struct FreeCompanyRef {
    pub id: FreeCompanyId,
    pub name: Option<String>,
    pub crest: Crest,
}

#[derive(Debug, Serialize)]
pub struct PvpTeamRef {
    pub id: PvpTeamId,
    pub name: Option<String>,
    pub crest: Crest,
}

#[derive(Debug, Serialize)]
pub struct ActiveClassJob {
    pub icon: Option<Url>,
    pub level: Option<u8>,
}

/// Selectors for the character page, one field per vendored file.
#[derive(Debug)]
pub struct CharacterSels {
    pub profile: ProfileSels,
    pub attributes: AttributeSels,
    pub gear: GearSels,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct ProfileSels {
    active_classjob: Sel,
    active_classjob_level: Sel,
    avatar: Sel,
    bio: Sel,
    free_company: FreeCompanyRefSels,
    grand_company: Sel,
    guardian_deity: IconNameSels,
    name: Sel,
    nameday: Sel,
    portrait: Sel,
    pvp_team: PvpTeamRefSels,
    race_clan_gender: Sel,
    server: Sel,
    title: Sel,
    town: IconNameSels,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct FreeCompanyRefSels {
    id: Sel,
    name: Sel,
    icon_layers: CrestSels,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct PvpTeamRefSels {
    /// The team link: `href` carries the ID, the text is the name.
    name: Sel,
    icon_layers: CrestSels,
}

/// Declares the attribute selectors and output with one field list.
macro_rules! attributes {
    ($($field:ident),* $(,)?) => {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub struct AttributeSels {
            $($field: Sel,)*
            mp_gp_cp_parameter_name: Sel,
        }

        #[derive(Debug, Serialize)]
        pub struct Attributes {
            $(pub $field: Option<u32>,)*
            /// Which resource `mp_gp_cp` is (MP, GP or CP).
            pub mp_gp_cp_parameter_name: Option<String>,
        }

        impl AttributeSels {
            fn parse(&self, scope: ElementRef<'_>) -> Attributes {
                Attributes {
                    $($field: self.$field.num(scope),)*
                    mp_gp_cp_parameter_name: self.mp_gp_cp_parameter_name.text(scope),
                }
            }
        }
    };
}

attributes!(
    strength,
    dexterity,
    vitality,
    intelligence,
    mind,
    critical_hit_rate,
    determination,
    direct_hit_rate,
    defense,
    magic_defense,
    attack_power,
    skill_speed,
    attack_magic_potency,
    healing_magic_potency,
    spell_speed,
    tenacity,
    piety,
    hp,
    mp_gp_cp,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all(serialize = "snake_case", deserialize = "UPPERCASE"))]
pub enum GearSlot {
    MainHand,
    OffHand,
    Head,
    Body,
    Hands,
    Waist,
    Legs,
    Feet,
    Earrings,
    Necklace,
    Bracelets,
    Ring1,
    Ring2,
    SoulCrystal,
}

#[derive(Debug, Serialize)]
pub struct GearPiece {
    pub name: String,
    /// Eorzea Database item ID.
    pub db_id: Option<String>,
    pub item_level: Option<u16>,
    pub glamour: Option<Glamour>,
    pub dye: Option<String>,
    pub materia: Vec<String>,
    pub crafter: Option<String>,
    /// The classes/jobs that can equip the item, as displayed.
    pub class_list: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Glamour {
    pub name: String,
    pub db_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GearSels(BTreeMap<GearSlot, GearPieceSels>);

/// The soul crystal only has `NAME`, `CLASS_LIST` and `ITEM_LEVEL`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct GearPieceSels {
    name: Sel,
    db_link: Option<Sel>,
    mirage_name: Option<Sel>,
    mirage_db_link: Option<Sel>,
    stain: Option<Sel>,
    materia_1: Option<Sel>,
    materia_2: Option<Sel>,
    materia_3: Option<Sel>,
    materia_4: Option<Sel>,
    materia_5: Option<Sel>,
    creator_name: Option<Sel>,
    class_list: Option<Sel>,
    item_level: Sel,
}

impl GearPieceSels {
    fn parse(&self, scope: ElementRef<'_>) -> Option<GearPiece> {
        let db_id = |sel: Option<&Sel>| {
            sel.and_then(|s| s.attr(scope, "href"))
                .and_then(|h| db_id(&h))
        };
        let text = |sel: Option<&Sel>| sel.and_then(|s| s.text(scope));
        let materia = [
            &self.materia_1,
            &self.materia_2,
            &self.materia_3,
            &self.materia_4,
            &self.materia_5,
        ]
        .into_iter()
        .filter_map(|m| m.as_ref()?.capture(scope, "Name"))
        .collect();

        Some(GearPiece {
            name: self.name.text(scope)?,
            db_id: db_id(self.db_link.as_ref()),
            item_level: self.item_level.digits(scope),
            glamour: text(self.mirage_name.as_ref()).map(|name| Glamour {
                name,
                db_id: db_id(self.mirage_db_link.as_ref()),
            }),
            dye: text(self.stain.as_ref()),
            materia,
            crafter: text(self.creator_name.as_ref()),
            class_list: text(self.class_list.as_ref()),
        })
    }
}

/// `/lodestone/playguide/db/item/<id>/` → `<id>`.
fn db_id(href: &str) -> Option<String> {
    let rest = href.split("/db/item/").nth(1)?;
    let id = rest.split('/').next()?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| id.to_owned())
}

impl Page for Character {
    const KIND: PageKind = PageKind::Character;
    type Request = CharacterId;

    fn url(base: &Url, id: &CharacterId) -> Url {
        page_url(base, ["character", &id.to_string()])
    }

    fn parse(doc: &Html, sel: &Selectors, id: &CharacterId) -> Result<Self, ParseError> {
        let s = doc.root_element();
        let p = &sel.character.profile;
        let rcg = p.race_clan_gender.captures(s);
        let rcg = |group| rcg.as_ref().and_then(|c| c.get(group));

        Ok(Self {
            id: *id,
            name: p.name.text(s).ok_or(ParseError::Missing("name"))?,
            title: p.title.text(s),
            world: World::parse(&p.server, s),
            race: rcg("Race"),
            tribe: rcg("Tribe"),
            gender: rcg("Gender").as_deref().and_then(Gender::from_symbol),
            nameday: p.nameday.text(s),
            guardian_deity: p.guardian_deity.parse(s),
            city_state: p.town.parse(s),
            grand_company: p.grand_company.captures(s).and_then(|c| {
                Some(GrandCompanyRank {
                    name: c.get("Name")?,
                    rank: c.get("Rank")?,
                })
            }),
            free_company: p
                .free_company
                .id
                .capture_num(s, "ID")
                .map(|fc_id| FreeCompanyRef {
                    id: FreeCompanyId(fc_id),
                    name: p.free_company.name.text(s),
                    crest: p.free_company.icon_layers.parse(s),
                }),
            pvp_team: p
                .pvp_team
                .name
                .capture(s, "ID")
                .and_then(|team| PvpTeamId::try_from(team).ok())
                .map(|team_id| PvpTeamRef {
                    id: team_id,
                    name: p.pvp_team.name.inner_text(s),
                    crest: p.pvp_team.icon_layers.parse(s),
                }),
            // An empty bio is rendered as a lone "-".
            bio: p.bio.text(s).filter(|b| b != "-"),
            avatar: p.avatar.url(s),
            portrait: p.portrait.url(s),
            active_class_job: ActiveClassJob {
                icon: p.active_classjob.url(s),
                level: p.active_classjob_level.capture_num(s, "Level"),
            },
            attributes: sel.character.attributes.parse(s),
            gear: sel
                .character
                .gear
                .0
                .iter()
                .filter_map(|(slot, piece)| Some((*slot, piece.parse(s)?)))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_db_ids() {
        assert_eq!(
            db_id("/lodestone/playguide/db/item/4a1b2c3d4e5/").as_deref(),
            Some("4a1b2c3d4e5")
        );
        assert_eq!(db_id("/lodestone/character/1/"), None);
    }
}
