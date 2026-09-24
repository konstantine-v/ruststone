//! `character/{id}/mount/` and `character/{id}/minion/`
//! (`profile/mount.json`, `profile/minion.json`). Mobile-only pages.

use scraper::Html;
use serde::{Deserialize, Serialize};
use url::Url;

use super::common::{IconName, IconNameSels};
use crate::lodestone::{
    ids::CharacterId,
    page_url,
    selector::{ListSel, Sel},
    Page, PageKind, ParseError, Selectors,
};

#[derive(Debug, Serialize)]
pub struct Collection {
    pub total: Option<u32>,
    pub items: Vec<IconName>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct CollectionSels {
    #[serde(alias = "MOUNTS", alias = "MINIONS")]
    items: ListSel<IconNameSels>,
    total: Sel,
}

impl CollectionSels {
    fn parse(&self, doc: &Html) -> Collection {
        let s = doc.root_element();
        Collection {
            total: self.total.num(s),
            items: self.items.parse(s, IconNameSels::parse),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct Mounts(pub Collection);

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct Minions(pub Collection);

impl Page for Mounts {
    const KIND: PageKind = PageKind::Mounts;
    type Request = CharacterId;

    fn url(base: &Url, id: &CharacterId) -> Url {
        page_url(base, ["character", &id.to_string(), "mount"])
    }

    fn parse(doc: &Html, sel: &Selectors, _id: &CharacterId) -> Result<Self, ParseError> {
        Ok(Self(sel.mounts.parse(doc)))
    }
}

impl Page for Minions {
    const KIND: PageKind = PageKind::Minions;
    type Request = CharacterId;

    fn url(base: &Url, id: &CharacterId) -> Url {
        page_url(base, ["character", &id.to_string(), "minion"])
    }

    fn parse(doc: &Html, sel: &Selectors, _id: &CharacterId) -> Result<Self, ParseError> {
        Ok(Self(sel.minions.parse(doc)))
    }
}
