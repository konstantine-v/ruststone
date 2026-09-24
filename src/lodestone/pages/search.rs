//! Search pages (`search/*.json`): one generic [`Search<E>`] page, one
//! [`SearchEntry`] impl per searchable kind.

use chrono::{DateTime, Utc};
use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};
use url::Url;

use super::common::{Crest, CrestSels, IconName, Paged, World};
use crate::lodestone::{
    ids::{
        CharacterId, CwlsId, FreeCompanyId, LinkshellId, PageNo, PvpTeamId, SearchName, WorldName,
    },
    page_url,
    selector::{PagedSel, Sel},
    Page, PageKind, ParseError, Selectors,
};

/// Query for every search endpoint. `world` wins over `dc` where a kind
/// supports both; data-center-only kinds (CWLS, PvP teams) ignore `world`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub name: SearchName,
    pub world: Option<WorldName>,
    pub dc: Option<WorldName>,
    #[serde(default)]
    pub page: PageNo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTarget {
    Character,
    FreeCompany,
    Linkshell,
    Cwls,
    PvpTeam,
}

impl SearchTarget {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Character => "search_character",
            Self::FreeCompany => "search_free_company",
            Self::Linkshell => "search_linkshell",
            Self::Cwls => "search_cwls",
            Self::PvpTeam => "search_pvp_team",
        }
    }

    const fn path(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::FreeCompany => "freecompany",
            Self::Linkshell => "linkshell",
            Self::Cwls => "crossworld_linkshell",
            Self::PvpTeam => "pvpteam",
        }
    }

    /// CWLS and PvP teams are cross-world: filtered by `dcname` only.
    const fn dc_only(self) -> bool {
        matches!(self, Self::Cwls | Self::PvpTeam)
    }
}

/// One searchable kind: which selectors it uses and how an entry is read.
pub trait SearchEntry: Serialize + Sized + Send + 'static {
    const TARGET: SearchTarget;
    type Sels;

    fn sels(sel: &Selectors) -> &PagedSel<Self::Sels>;
    fn parse(sels: &Self::Sels, el: ElementRef<'_>) -> Option<Self>;
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct Search<E>(pub Paged<E>);

impl<E: SearchEntry> Page for Search<E> {
    const KIND: PageKind = PageKind::Search(E::TARGET);
    type Request = SearchQuery;

    fn url(base: &Url, q: &SearchQuery) -> Url {
        let target = E::TARGET;
        let mut url = page_url(base, [target.path()]);
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", q.name.as_ref());
            if target.dc_only() {
                if let Some(dc) = &q.dc {
                    pairs.append_pair("dcname", dc.as_ref());
                }
            } else if let Some(world) = &q.world {
                pairs.append_pair("worldname", world.as_ref());
            } else if let Some(dc) = &q.dc {
                pairs.append_pair("worldname", &format!("_dc_{}", dc.as_ref()));
            }
            pairs.append_pair("page", &q.page.to_string());
        }
        url
    }

    fn parse(doc: &Html, sel: &Selectors, _q: &SearchQuery) -> Result<Self, ParseError> {
        Ok(Self(E::sels(sel).parse(doc, E::parse)))
    }
}

/// Selectors for every search kind.
#[derive(Debug)]
pub struct SearchSels {
    pub character: PagedSel<CharacterEntrySels>,
    pub free_company: PagedSel<FreeCompanyEntrySels>,
    pub linkshell: PagedSel<LinkshellEntrySels>,
    pub cwls: PagedSel<CwlsEntrySels>,
    pub pvp_team: PagedSel<PvpTeamEntrySels>,
}

// ----- characters -----

#[derive(Debug, Serialize)]
pub struct CharacterEntry {
    pub id: CharacterId,
    pub name: String,
    pub avatar: Option<Url>,
    pub world: Option<World>,
    pub lang: Option<String>,
    pub gc_rank: Option<IconName>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct CharacterEntrySels {
    avatar: Sel,
    id: Sel,
    lang: Sel,
    name: Sel,
    rank: Sel,
    rank_icon: Sel,
    server: Sel,
}

impl SearchEntry for CharacterEntry {
    const TARGET: SearchTarget = SearchTarget::Character;
    type Sels = CharacterEntrySels;

    fn sels(sel: &Selectors) -> &PagedSel<Self::Sels> {
        &sel.search.character
    }

    fn parse(s: &Self::Sels, el: ElementRef<'_>) -> Option<Self> {
        Some(Self {
            id: CharacterId(s.id.capture_num(el, "ID")?),
            name: s.name.text(el)?,
            avatar: s.avatar.url(el),
            world: World::parse(&s.server, el),
            lang: s.lang.text(el),
            gc_rank: s.rank.capture(el, "RankName").map(|name| IconName {
                name,
                icon: s.rank_icon.url(el),
            }),
        })
    }
}

// ----- free companies -----

#[derive(Debug, Serialize)]
pub struct FreeCompanyEntry {
    pub id: FreeCompanyId,
    pub name: String,
    pub world: Option<World>,
    pub crest: Crest,
    pub grand_company: Option<String>,
    pub active: Option<String>,
    pub active_members: Option<u16>,
    pub recruitment: Option<String>,
    pub estate: Option<String>,
    pub formed: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct FreeCompanyEntrySels {
    crest_layers: CrestSels,
    id: Sel,
    grand_company: Sel,
    name: Sel,
    server: Sel,
    active: Sel,
    active_members: Sel,
    recruitment_open: Sel,
    estate_built: Sel,
    formed: Sel,
}

impl SearchEntry for FreeCompanyEntry {
    const TARGET: SearchTarget = SearchTarget::FreeCompany;
    type Sels = FreeCompanyEntrySels;

    fn sels(sel: &Selectors) -> &PagedSel<Self::Sels> {
        &sel.search.free_company
    }

    fn parse(s: &Self::Sels, el: ElementRef<'_>) -> Option<Self> {
        Some(Self {
            id: FreeCompanyId(s.id.capture_num(el, "ID")?),
            name: s.name.text(el)?,
            world: World::parse(&s.server, el),
            crest: s.crest_layers.parse(el),
            grand_company: s.grand_company.text(el),
            active: s.active.capture(el, "State"),
            active_members: s.active_members.num(el),
            recruitment: s.recruitment_open.capture(el, "State"),
            estate: s.estate_built.text(el),
            formed: s.formed.timestamp(el),
        })
    }
}

// ----- linkshells -----

#[derive(Debug, Serialize)]
pub struct LinkshellEntry {
    pub id: LinkshellId,
    pub name: String,
    pub world: Option<World>,
    pub active_members: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct LinkshellEntrySels {
    id: Sel,
    name: Sel,
    server: Sel,
    active_members: Sel,
}

impl SearchEntry for LinkshellEntry {
    const TARGET: SearchTarget = SearchTarget::Linkshell;
    type Sels = LinkshellEntrySels;

    fn sels(sel: &Selectors) -> &PagedSel<Self::Sels> {
        &sel.search.linkshell
    }

    fn parse(s: &Self::Sels, el: ElementRef<'_>) -> Option<Self> {
        Some(Self {
            id: LinkshellId(s.id.capture_num(el, "ID")?),
            name: s.name.text(el)?,
            world: World::parse(&s.server, el),
            active_members: s.active_members.num(el),
        })
    }
}

// ----- cross-world linkshells -----

#[derive(Debug, Serialize)]
pub struct CwlsEntry {
    pub id: CwlsId,
    pub name: String,
    pub data_center: Option<String>,
    pub active_members: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct CwlsEntrySels {
    id: Sel,
    name: Sel,
    dc: Sel,
    active_members: Sel,
}

impl SearchEntry for CwlsEntry {
    const TARGET: SearchTarget = SearchTarget::Cwls;
    type Sels = CwlsEntrySels;

    fn sels(sel: &Selectors) -> &PagedSel<Self::Sels> {
        &sel.search.cwls
    }

    fn parse(s: &Self::Sels, el: ElementRef<'_>) -> Option<Self> {
        Some(Self {
            id: CwlsId::try_from(s.id.capture(el, "ID")?).ok()?,
            name: s.name.text(el)?,
            data_center: s.dc.text(el),
            active_members: s.active_members.num(el),
        })
    }
}

// ----- PvP teams -----

#[derive(Debug, Serialize)]
pub struct PvpTeamEntry {
    pub id: PvpTeamId,
    pub name: String,
    pub data_center: Option<String>,
    pub crest: Crest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct PvpTeamEntrySels {
    crest_layers: CrestSels,
    id: Sel,
    name: Sel,
    dc: Sel,
}

impl SearchEntry for PvpTeamEntry {
    const TARGET: SearchTarget = SearchTarget::PvpTeam;
    type Sels = PvpTeamEntrySels;

    fn sels(sel: &Selectors) -> &PagedSel<Self::Sels> {
        &sel.search.pvp_team
    }

    fn parse(s: &Self::Sels, el: ElementRef<'_>) -> Option<Self> {
        Some(Self {
            id: PvpTeamId::try_from(s.id.capture(el, "ID")?).ok()?,
            name: s.name.text(el)?,
            data_center: s.dc.text(el),
            crest: s.crest_layers.parse(el),
        })
    }
}
