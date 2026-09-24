//! `freecompany/{id}/` (`freecompany.json` + `focus.json` + `seeking.json` +
//! `reputation.json`, all from one page) and `freecompany/{id}/member/`.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use scraper::{ElementRef, Html};
use serde::{de::IgnoredAny, Deserialize, Serialize};
use url::Url;

use super::common::{Crest, CrestSels, Member, MemberSels, Paged, World};
use crate::lodestone::{
    ids::{FreeCompanyId, PageNo},
    page_url,
    selector::{PagedSel, Sel},
    Page, PageKind, ParseError, Selectors,
};

#[derive(Debug, Serialize)]
pub struct FreeCompany {
    pub id: FreeCompanyId,
    pub name: String,
    pub tag: Option<String>,
    pub slogan: Option<String>,
    pub world: Option<World>,
    pub crest: Crest,
    pub formed: Option<DateTime<Utc>>,
    pub grand_company: Option<GrandCompanyStanding>,
    pub rank: Option<u8>,
    pub ranking: Ranking,
    pub active_state: Option<String>,
    pub recruitment: Option<String>,
    pub active_member_count: Option<u16>,
    /// `None` when the company has no estate.
    pub estate: Option<Estate>,
    /// Empty when the company has not specified a focus.
    pub focus: Vec<Toggle<Focus>>,
    pub seeking: Vec<Toggle<Role>>,
    pub reputation: Vec<Reputation>,
}

#[derive(Debug, Serialize)]
pub struct GrandCompanyStanding {
    pub name: String,
    pub standing: String,
}

#[derive(Debug, Serialize)]
pub struct Ranking {
    pub weekly: Option<u32>,
    pub monthly: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct Estate {
    pub name: Option<String>,
    pub plot: Option<String>,
    pub greeting: Option<String>,
}

/// One focus/seeking icon: whether the company has it switched on.
#[derive(Debug, Serialize)]
pub struct Toggle<K> {
    pub kind: K,
    pub name: String,
    pub icon: Option<Url>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all(serialize = "snake_case", deserialize = "UPPERCASE"))]
pub enum Focus {
    Rp,
    Leveling,
    Casual,
    Hardcore,
    Dungeons,
    Guildhests,
    Trials,
    Raids,
    Pvp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all(serialize = "snake_case", deserialize = "UPPERCASE"))]
pub enum Role {
    Tank,
    Healer,
    Dps,
    Crafter,
    Gatherer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all(serialize = "snake_case", deserialize = "UPPERCASE"))]
pub enum GrandCompany {
    Maelstrom,
    Adders,
    Flames,
}

#[derive(Debug, Serialize)]
pub struct Reputation {
    pub grand_company: GrandCompany,
    pub name: String,
    pub rank: Option<String>,
    /// Progress towards the next rank, in percent.
    pub progress: Option<u8>,
}

/// Selectors for the free company page, one field per vendored file.
#[derive(Debug)]
pub struct FreeCompanySels {
    pub profile: ProfileSels,
    pub focus: ToggleGroupSels<Focus>,
    pub seeking: ToggleGroupSels<Role>,
    pub reputation: BTreeMap<GrandCompany, ReputationSels>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct ProfileSels {
    active_state: Sel,
    active_member_count: Sel,
    crest_layers: CrestSels,
    estate: EstateSels,
    formed: Sel,
    grand_company: Sel,
    name: Sel,
    rank: Sel,
    ranking: RankingSels,
    recruitment: Sel,
    server: Sel,
    slogan: Sel,
    tag: Sel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct EstateSels {
    no_estate: Sel,
    greeting: Sel,
    name: Sel,
    plot: Sel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct RankingSels {
    weekly: Sel,
    monthly: Sel,
}

/// `focus.json` / `seeking.json`: one `NAME`/`ICON`/`STATUS` group per kind.
#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "K: Deserialize<'de> + Ord"))]
pub struct ToggleGroupSels<K> {
    /// A plain selector rather than a group; an empty list already says it.
    #[serde(rename = "NOT_SPECIFIED")]
    _not_specified: IgnoredAny,
    #[serde(flatten)]
    entries: BTreeMap<K, ToggleSels>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct ToggleSels {
    name: Sel,
    icon: Sel,
    status: Sel,
}

impl<K: Copy> ToggleGroupSels<K> {
    fn parse(&self, scope: ElementRef<'_>) -> Vec<Toggle<K>> {
        self.entries
            .iter()
            .filter_map(|(kind, t)| {
                Some(Toggle {
                    kind: *kind,
                    name: t.name.text(scope)?,
                    icon: t.icon.url(scope),
                    // The icon's class carries `--off` when disabled.
                    enabled: t.status.captures(scope).is_none(),
                })
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct ReputationSels {
    name: Sel,
    progress: Sel,
    rank: Sel,
}

impl Page for FreeCompany {
    const KIND: PageKind = PageKind::FreeCompany;
    type Request = FreeCompanyId;

    fn url(base: &Url, id: &FreeCompanyId) -> Url {
        page_url(base, ["freecompany", &id.to_string()])
    }

    fn parse(doc: &Html, sel: &Selectors, id: &FreeCompanyId) -> Result<Self, ParseError> {
        let s = doc.root_element();
        let fc = &sel.free_company;
        let p = &fc.profile;
        let estate = &p.estate;

        Ok(Self {
            id: *id,
            name: p.name.text(s).ok_or(ParseError::Missing("name"))?,
            tag: p.tag.text(s),
            slogan: p.slogan.text(s),
            world: World::parse(&p.server, s),
            crest: p.crest_layers.parse(s),
            formed: p.formed.timestamp(s),
            grand_company: p.grand_company.captures(s).and_then(|c| {
                Some(GrandCompanyStanding {
                    name: c.get("Name")?,
                    standing: c.get("Rank")?,
                })
            }),
            rank: p.rank.num(s),
            ranking: Ranking {
                weekly: p.ranking.weekly.capture_num(s, "Rank"),
                monthly: p.ranking.monthly.capture_num(s, "Rank"),
            },
            active_state: p.active_state.capture(s, "ActiveState"),
            recruitment: p.recruitment.capture(s, "ActiveState"),
            active_member_count: p.active_member_count.num(s),
            estate: (!estate.no_estate.exists(s)).then(|| Estate {
                name: estate.name.text(s),
                plot: estate.plot.text(s),
                greeting: estate.greeting.text(s),
            }),
            focus: fc.focus.parse(s),
            seeking: fc.seeking.parse(s),
            reputation: fc
                .reputation
                .iter()
                .filter_map(|(gc, r)| {
                    Some(Reputation {
                        grand_company: *gc,
                        name: r.name.text(s)?,
                        rank: r.rank.text(s),
                        progress: r.progress.capture_num(s, "Progress"),
                    })
                })
                .collect(),
        })
    }
}

/// One page of a free company's member list.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct FreeCompanyMembers(pub Paged<Member>);

#[derive(Debug, Clone, Copy, Serialize)]
pub struct MembersRequest {
    pub id: FreeCompanyId,
    pub page: PageNo,
}

pub type MembersSels = PagedSel<MemberSels>;

impl Page for FreeCompanyMembers {
    const KIND: PageKind = PageKind::FreeCompanyMembers;
    type Request = MembersRequest;

    fn url(base: &Url, req: &MembersRequest) -> Url {
        let mut url = page_url(base, ["freecompany", &req.id.to_string(), "member"]);
        url.query_pairs_mut()
            .append_pair("page", &req.page.to_string());
        url
    }

    fn parse(doc: &Html, sel: &Selectors, _req: &MembersRequest) -> Result<Self, ParseError> {
        Ok(Self(sel.free_company_members.parse(doc, MemberSels::parse)))
    }
}
