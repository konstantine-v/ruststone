//! `character/{id}/class_job/` (`profile/classjob.json`).

use std::collections::BTreeMap;

use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::lodestone::{
    ids::CharacterId, page_url, selector::Sel, Page, PageKind, ParseError, Selectors,
};

#[derive(Debug, Serialize)]
pub struct ClassJobs {
    pub jobs: Vec<JobProgress>,
    pub bozja: Option<Bozja>,
    pub eureka: Option<Eureka>,
}

/// Every class/job the selector file knows. A job added upstream must be
/// added here, otherwise loading the selectors fails (in tests and at boot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all(serialize = "snake_case", deserialize = "UPPERCASE"))]
pub enum Job {
    Paladin,
    Warrior,
    DarkKnight,
    Gunbreaker,
    WhiteMage,
    Scholar,
    Astrologian,
    Sage,
    Monk,
    Dragoon,
    Ninja,
    Samurai,
    Reaper,
    Viper,
    Beastmaster,
    Bard,
    Machinist,
    Dancer,
    BlackMage,
    Summoner,
    RedMage,
    Pictomancer,
    BlueMage,
    Carpenter,
    Blacksmith,
    Armorer,
    Goldsmith,
    Leatherworker,
    Weaver,
    Alchemist,
    Culinarian,
    Miner,
    Botanist,
    Fisher,
}

#[derive(Debug, Serialize)]
pub struct JobProgress {
    pub job: Job,
    /// `None` when the class has not been unlocked.
    pub level: Option<u8>,
    /// The class or job name as displayed (reveals whether the job is unlocked).
    pub unlock_state: Option<String>,
    pub exp: Option<Exp>,
}

#[derive(Debug, Serialize)]
pub struct Exp {
    pub current: u64,
    pub max: u64,
}

#[derive(Debug, Serialize)]
pub struct Bozja {
    pub name: Option<String>,
    pub level: Option<u8>,
    pub mettle: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct Eureka {
    pub name: Option<String>,
    pub level: Option<u8>,
    pub exp: Option<Exp>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct ClassJobSels {
    bozja: BozjaSels,
    eureka: EurekaSels,
    #[serde(flatten)]
    jobs: BTreeMap<Job, JobSels>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct JobSels {
    level: Sel,
    unlockstate: Sel,
    exp: Sel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct BozjaSels {
    level: Sel,
    mettle: Sel,
    name: Sel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct EurekaSels {
    exp: Sel,
    level: Sel,
    name: Sel,
}

fn exp(sel: &Sel, scope: ElementRef<'_>) -> Option<Exp> {
    let caps = sel.captures(scope)?;
    Some(Exp {
        current: caps.num("CurrentEXP")?,
        max: caps.num("MaxEXP")?,
    })
}

impl Page for ClassJobs {
    const KIND: PageKind = PageKind::ClassJobs;
    type Request = CharacterId;

    fn url(base: &Url, id: &CharacterId) -> Url {
        page_url(base, ["character", &id.to_string(), "class_job"])
    }

    fn parse(doc: &Html, sel: &Selectors, _id: &CharacterId) -> Result<Self, ParseError> {
        let s = doc.root_element();
        let cj = &sel.class_jobs;
        let jobs: Vec<_> = cj
            .jobs
            .iter()
            .map(|(job, js)| JobProgress {
                job: *job,
                level: js.level.num(s),
                unlock_state: js.unlockstate.text(s),
                exp: exp(&js.exp, s),
            })
            .collect();
        if jobs.iter().all(|j| j.unlock_state.is_none()) {
            return Err(ParseError::Missing("class jobs"));
        }

        let bozja = Bozja {
            name: cj.bozja.name.text(s),
            level: cj.bozja.level.num(s),
            mettle: cj.bozja.mettle.capture_num(s, "Mettle"),
        };
        let eureka = Eureka {
            name: cj.eureka.name.text(s),
            level: cj.eureka.level.num(s),
            exp: exp(&cj.eureka.exp, s),
        };
        Ok(Self {
            jobs,
            bozja: bozja.name.is_some().then_some(bozja),
            eureka: eureka.name.is_some().then_some(eureka),
        })
    }
}
