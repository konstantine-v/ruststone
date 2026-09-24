//! Output types and selector groups shared by several pages.

use scraper::ElementRef;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::lodestone::{ids::CharacterId, selector::Sel};

/// A page of a Lodestone list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paged<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

impl<T> Paged<T> {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            items: Vec::new(),
            pagination: Pagination::new(1, 0),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pagination {
    pub page: u16,
    pub total_pages: u16,
    pub next: Option<u16>,
    pub prev: Option<u16>,
}

impl Pagination {
    #[must_use]
    pub const fn new(page: u16, total_pages: u16) -> Self {
        Self {
            page,
            total_pages,
            next: if page < total_pages {
                Some(page + 1)
            } else {
                None
            },
            prev: if page > 1 { Some(page - 1) } else { None },
        }
    }
}

/// `World [DataCenter]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub name: String,
    pub data_center: String,
}

impl World {
    /// Reads a `(?P<World>…) [(?P<DC>…)]` selector.
    #[must_use]
    pub fn parse(sel: &Sel, scope: ElementRef<'_>) -> Option<Self> {
        let caps = sel.captures(scope)?;
        Some(Self {
            name: caps.get("World")?,
            data_center: caps.get("DC")?,
        })
    }
}

/// Free company / PvP team crest, as up to three stacked image layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crest {
    pub bottom: Option<Url>,
    pub middle: Option<Url>,
    pub top: Option<Url>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct CrestSels {
    bottom: Sel,
    middle: Sel,
    top: Sel,
}

impl CrestSels {
    #[must_use]
    pub fn parse(&self, scope: ElementRef<'_>) -> Crest {
        Crest {
            bottom: self.bottom.url(scope),
            middle: self.middle.url(scope),
            top: self.top.url(scope),
        }
    }
}

/// A named thing with an icon (town, deity, mount, rank, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconName {
    pub name: String,
    pub icon: Option<Url>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct IconNameSels {
    name: Sel,
    icon: Sel,
}

impl IconNameSels {
    #[must_use]
    pub fn parse(&self, scope: ElementRef<'_>) -> Option<IconName> {
        Some(IconName {
            name: self.name.text(scope)?,
            icon: self.icon.url(scope),
        })
    }
}

/// A character listed on a free company, linkshell, CWLS or PvP team page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: CharacterId,
    pub name: String,
    pub avatar: Option<Url>,
    pub world: Option<World>,
    /// Grand company rank.
    pub gc_rank: Option<IconName>,
    /// Rank inside the group (free company rank, linkshell master, …).
    pub role: Option<IconName>,
    /// PvP team pages only.
    pub matches: Option<u32>,
}

/// Member entry selectors; the group-specific rank keys are aliased onto `role`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct MemberSels {
    avatar: Sel,
    id: Sel,
    name: Sel,
    rank: Option<Sel>,
    rank_icon: Option<Sel>,
    #[serde(alias = "FC_RANK", alias = "LINKSHELL_RANK")]
    role: Option<Sel>,
    #[serde(alias = "FC_RANK_ICON", alias = "LINKSHELL_RANK_ICON")]
    role_icon: Option<Sel>,
    matches: Option<Sel>,
    server: Sel,
}

impl MemberSels {
    #[must_use]
    pub fn parse(&self, el: ElementRef<'_>) -> Option<Member> {
        let icon_name = |name: Option<&Sel>, icon: Option<&Sel>, group: Option<&str>| {
            let name = name?;
            let name = group.map_or_else(|| name.text(el), |g| name.capture(el, g))?;
            Some(IconName {
                name,
                icon: icon.and_then(|s| s.url(el)),
            })
        };
        Some(Member {
            id: CharacterId(self.id.capture_num(el, "ID")?),
            name: self.name.text(el)?,
            avatar: self.avatar.url(el),
            world: World::parse(&self.server, el),
            gc_rank: icon_name(
                self.rank.as_ref(),
                self.rank_icon.as_ref(),
                Some("RankName"),
            ),
            role: icon_name(self.role.as_ref(), self.role_icon.as_ref(), None),
            matches: self.matches.as_ref().and_then(|s| s.num(el)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_links() {
        let p = Pagination::new(1, 3);
        assert_eq!((p.prev, p.next), (None, Some(2)));
        let p = Pagination::new(3, 3);
        assert_eq!((p.prev, p.next), (Some(2), None));
        let p = Pagination::new(1, 0);
        assert_eq!((p.prev, p.next), (None, None));
    }
}
