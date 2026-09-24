//! Selector drift checks against the real Lodestone. Ignored by default:
//! `cargo test -- --ignored --nocapture`. IDs are discovered through search,
//! so nothing here goes stale.

use loco_rs::cache::{drivers::null, Cache};
use ruststone::{
    lodestone::{
        ids::{PageNo, SearchName},
        pages::{
            achievements::{Achievements, AchievementsRequest},
            character::Character,
            class_jobs::ClassJobs,
            collections::Mounts,
            free_company::{FreeCompany, FreeCompanyMembers, MembersRequest},
            search::{CharacterEntry, FreeCompanyEntry, Search, SearchQuery},
        },
        Cached, Lodestone, LodestoneError, Page, Selectors,
    },
    settings::Settings,
};
use serde_json::Value;

fn client() -> (Lodestone, Cache) {
    let lodestone = Lodestone::new(Settings::default(), Selectors::load().unwrap()).unwrap();
    (lodestone, Cache::new(null::new()))
}

fn json<P>(page: &Cached<P>) -> Value {
    serde_json::from_str(page.json()).unwrap()
}

fn query(name: &str) -> SearchQuery {
    SearchQuery {
        name: SearchName::try_from(name.to_owned()).unwrap(),
        world: None,
        dc: None,
        page: PageNo::FIRST,
    }
}

async fn fetch<P: Page>(l: &Lodestone, c: &Cache, req: P::Request) -> Value {
    let page = l.fetch::<P>(c, req).await.unwrap();
    let value = json(&page);
    println!(
        "{}: {}",
        P::KIND.as_str(),
        serde_json::to_string_pretty(&value).unwrap()
    );
    value
}

#[tokio::test]
#[ignore = "hits the live Lodestone"]
async fn character_pages_parse() {
    let (l, c) = client();
    let found = fetch::<Search<CharacterEntry>>(&l, &c, query("Alpha")).await;
    let id: u64 = serde_json::from_value(found["items"][0]["id"].clone()).expect("a search result");
    let id = ruststone::lodestone::ids::CharacterId(id);

    let character = fetch::<Character>(&l, &c, id).await;
    assert!(character["name"].is_string());
    assert!(character["world"]["data_center"].is_string());
    assert!(
        character["race"].is_string(),
        "race/clan/gender regex drifted"
    );
    assert!(character["active_class_job"]["level"].is_number());
    assert!(
        character["attributes"]["hp"].is_number(),
        "attributes drifted"
    );
    assert!(
        !character["gear"].as_object().unwrap().is_empty(),
        "gearset drifted"
    );

    let jobs = fetch::<ClassJobs>(&l, &c, id).await;
    assert!(jobs["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|j| j["level"].is_number()));

    let mounts = fetch::<Mounts>(&l, &c, id).await;
    assert!(mounts["total"].is_number() || mounts["items"].is_array());

    match l
        .fetch::<Achievements>(
            &c,
            AchievementsRequest {
                id,
                page: PageNo::FIRST,
            },
        )
        .await
    {
        Ok(page) => assert!(json(&page)["pagination"]["page"].is_number()),
        Err(LodestoneError::Private) => println!("achievements are private"),
        Err(e) => panic!("{e}"),
    }
}

#[tokio::test]
#[ignore = "hits the live Lodestone"]
async fn free_company_pages_parse() {
    let (l, c) = client();
    let found = fetch::<Search<FreeCompanyEntry>>(&l, &c, query("Moogle")).await;
    let id: u64 = serde_json::from_value(found["items"][0]["id"].clone()).expect("a search result");
    let id = ruststone::lodestone::ids::FreeCompanyId(id);

    let fc = fetch::<FreeCompany>(&l, &c, id).await;
    assert!(fc["name"].is_string());
    assert!(fc["world"]["name"].is_string());
    assert!(fc["crest"]["bottom"].is_string() || fc["crest"]["top"].is_string());

    let members = fetch::<FreeCompanyMembers>(
        &l,
        &c,
        MembersRequest {
            id,
            page: PageNo::FIRST,
        },
    )
    .await;
    assert!(!members["items"].as_array().unwrap().is_empty());
    assert!(
        members["items"][0]["role"]["name"].is_string(),
        "member rank drifted"
    );
}
