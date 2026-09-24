//! `character/{id}/achievement/?page=` (`profile/achievements.json`).

use chrono::{DateTime, Utc};
use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};
use url::Url;

use super::common::Paged;
use crate::lodestone::{
    ids::{CharacterId, PageNo},
    page_url,
    selector::{PagedSel, Sel},
    Page, PageKind, ParseError, Selectors,
};

#[derive(Debug, Serialize)]
pub struct Achievements {
    pub points: Option<u32>,
    pub total: Option<u32>,
    #[serde(flatten)]
    pub list: Paged<Achievement>,
}

#[derive(Debug, Serialize)]
pub struct Achievement {
    pub id: u32,
    pub name: Option<String>,
    pub unlocked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct AchievementsRequest {
    pub id: CharacterId,
    pub page: PageNo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct AchievementsSels {
    #[serde(flatten)]
    list: PagedSel<EntrySels>,
    total_achievements: Sel,
    achievement_points: Sel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct EntrySels {
    id: Sel,
    name: Sel,
    time: Sel,
}

impl EntrySels {
    fn parse(&self, el: ElementRef<'_>) -> Option<Achievement> {
        // The German site phrases entries differently, hence two groups.
        let name = self
            .name
            .captures(el)
            .and_then(|c| c.get("Name").or_else(|| c.get("NameDE")));
        Some(Achievement {
            id: self.id.capture_num(el, "ID")?,
            name,
            unlocked_at: self.time.timestamp(el),
        })
    }
}

impl Page for Achievements {
    const KIND: PageKind = PageKind::Achievements;
    type Request = AchievementsRequest;

    fn url(base: &Url, req: &AchievementsRequest) -> Url {
        let mut url = page_url(base, ["character", &req.id.to_string(), "achievement"]);
        url.query_pairs_mut()
            .append_pair("page", &req.page.to_string());
        url
    }

    fn parse(doc: &Html, sel: &Selectors, _req: &AchievementsRequest) -> Result<Self, ParseError> {
        let s = doc.root_element();
        let a = &sel.achievements;
        Ok(Self {
            points: a.achievement_points.num(s),
            total: a.total_achievements.capture_num(s, "TotalAchievements"),
            list: a.list.parse(doc, EntrySels::parse),
        })
    }
}
