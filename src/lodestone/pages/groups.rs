//! Group pages whose member list is on the same page:
//! `linkshell/{id}/`, `crossworld_linkshell/{id}/` and `pvpteam/{id}/`.

use chrono::{DateTime, Utc};
use scraper::Html;
use serde::{Deserialize, Serialize};
use url::Url;

use super::common::{Crest, CrestSels, Member, MemberSels, Paged};
use crate::lodestone::{
    ids::{CwlsId, LinkshellId, PageNo, PvpTeamId},
    page_url,
    selector::{ListSel, PagedSel, Sel},
    Page, PageKind, ParseError, Selectors,
};

#[derive(Debug, Clone, Serialize)]
pub struct GroupRequest<Id> {
    pub id: Id,
    pub page: PageNo,
}

fn paged_url(base: &Url, kind: &str, id: &str, page: PageNo) -> Url {
    let mut url = page_url(base, [kind, id]);
    url.query_pairs_mut().append_pair("page", &page.to_string());
    url
}

// ----- linkshell -----

#[derive(Debug, Serialize)]
pub struct Linkshell {
    pub id: LinkshellId,
    pub name: String,
    pub members: Paged<Member>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct LinkshellSels {
    name: Sel,
}

impl Page for Linkshell {
    const KIND: PageKind = PageKind::Linkshell;
    type Request = GroupRequest<LinkshellId>;

    fn url(base: &Url, req: &Self::Request) -> Url {
        paged_url(base, "linkshell", &req.id.to_string(), req.page)
    }

    fn parse(doc: &Html, sel: &Selectors, req: &Self::Request) -> Result<Self, ParseError> {
        Ok(Self {
            id: req.id,
            name: sel
                .linkshell
                .name
                .text(doc.root_element())
                .ok_or(ParseError::Missing("name"))?,
            members: sel.linkshell_members.parse(doc, MemberSels::parse),
        })
    }
}

// ----- cross-world linkshell -----

#[derive(Debug, Serialize)]
pub struct Cwls {
    pub id: CwlsId,
    pub name: String,
    pub data_center: Option<String>,
    pub members: Paged<Member>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct CwlsSels {
    name: Sel,
    dc: Sel,
}

impl Page for Cwls {
    const KIND: PageKind = PageKind::CrossworldLinkshell;
    type Request = GroupRequest<CwlsId>;

    fn url(base: &Url, req: &Self::Request) -> Url {
        paged_url(base, "crossworld_linkshell", &req.id.to_string(), req.page)
    }

    fn parse(doc: &Html, sel: &Selectors, req: &Self::Request) -> Result<Self, ParseError> {
        let s = doc.root_element();
        Ok(Self {
            id: req.id.clone(),
            name: sel
                .cwls
                .name
                .capture(s, "Name")
                .ok_or(ParseError::Missing("name"))?,
            data_center: sel.cwls.dc.text(s),
            members: sel.cwls_members.parse(doc, MemberSels::parse),
        })
    }
}

// ----- PvP team -----

#[derive(Debug, Serialize)]
pub struct PvpTeam {
    pub id: PvpTeamId,
    pub name: String,
    pub data_center: Option<String>,
    pub formed: Option<DateTime<Utc>>,
    pub crest: Crest,
    pub members: Vec<Member>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct PvpTeamSels {
    name: Sel,
    dc: Sel,
    formed: Sel,
    crest_layers: CrestSels,
}

/// `pvpteam/members.json`: a single, unpaginated list.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct PvpTeamMembersSels {
    root: Sel,
    entry: ListSel<MemberSels>,
}

impl Page for PvpTeam {
    const KIND: PageKind = PageKind::PvpTeam;
    type Request = PvpTeamId;

    fn url(base: &Url, id: &PvpTeamId) -> Url {
        page_url(base, ["pvpteam", &id.to_string()])
    }

    fn parse(doc: &Html, sel: &Selectors, id: &PvpTeamId) -> Result<Self, ParseError> {
        let s = doc.root_element();
        let team = &sel.pvp_team;
        let members = &sel.pvp_team_members;
        Ok(Self {
            id: id.clone(),
            name: team.name.text(s).ok_or(ParseError::Missing("name"))?,
            data_center: team.dc.text(s),
            formed: team.formed.timestamp(s),
            crest: team.crest_layers.parse(s),
            members: members
                .root
                .first(s)
                .map(|root| members.entry.parse(root, MemberSels::parse))
                .unwrap_or_default(),
        })
    }
}

pub type GroupMembersSels = PagedSel<MemberSels>;
