use axum::extract::Query;
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};

use super::{cached_json, include, respond, Included, PageQuery};
use crate::lodestone::{
    ids::{CharacterId, CommaList, PageNo},
    pages::{
        achievements::{Achievements, AchievementsRequest},
        character::Character,
        class_jobs::ClassJobs,
        collections::{Minions, Mounts},
        search::{CharacterEntry, Search, SearchQuery},
    },
    Cached, Lodestone, Page,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    ClassJobs,
    Achievements,
    Mounts,
    Minions,
}

#[derive(Debug, Deserialize)]
pub struct ShowQuery {
    #[serde(default)]
    pub include: CommaList<Section>,
}

#[derive(Serialize)]
struct CharacterResponse {
    character: Cached<Character>,
    #[serde(skip_serializing_if = "Included::is_not_requested")]
    class_jobs: Included<Cached<ClassJobs>>,
    #[serde(skip_serializing_if = "Included::is_not_requested")]
    achievements: Included<Cached<Achievements>>,
    #[serde(skip_serializing_if = "Included::is_not_requested")]
    mounts: Included<Cached<Mounts>>,
    #[serde(skip_serializing_if = "Included::is_not_requested")]
    minions: Included<Cached<Minions>>,
}

/// `GET /api/characters?name=&world=|dc=&page=`
#[debug_handler]
async fn search(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Query(q): Query<SearchQuery>,
) -> Result<Response> {
    respond::<Search<CharacterEntry>>(&lodestone, &ctx, q).await
}

/// `GET /api/characters/{id}?include=class_jobs,achievements,mounts,minions`
///
/// Included sections are fetched concurrently with the profile.
#[debug_handler]
async fn show(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<CharacterId>,
    Query(q): Query<ShowQuery>,
) -> Result<Response> {
    let cache = &ctx.cache;
    let wants = |s| q.include.contains(&s);
    let (character, class_jobs, achievements, mounts, minions) = tokio::try_join!(
        lodestone.fetch::<Character>(cache, id),
        include(
            wants(Section::ClassJobs),
            lodestone.fetch::<ClassJobs>(cache, id)
        ),
        include(
            wants(Section::Achievements),
            lodestone.fetch::<Achievements>(
                cache,
                AchievementsRequest {
                    id,
                    page: PageNo::FIRST
                }
            ),
        ),
        include(wants(Section::Mounts), lodestone.fetch::<Mounts>(cache, id)),
        include(
            wants(Section::Minions),
            lodestone.fetch::<Minions>(cache, id)
        ),
    )?;

    let max_age = q
        .include
        .0
        .iter()
        .map(|s| match s {
            Section::ClassJobs => ClassJobs::KIND,
            Section::Achievements => Achievements::KIND,
            Section::Mounts => Mounts::KIND,
            Section::Minions => Minions::KIND,
        })
        .map(|kind| lodestone.ttl(kind))
        .fold(lodestone.ttl(Character::KIND), std::cmp::min);

    cached_json(
        max_age,
        CharacterResponse {
            character,
            class_jobs,
            achievements,
            mounts,
            minions,
        },
    )
}

/// `GET /api/characters/{id}/class_jobs`
#[debug_handler]
async fn class_jobs(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<CharacterId>,
) -> Result<Response> {
    respond::<ClassJobs>(&lodestone, &ctx, id).await
}

/// `GET /api/characters/{id}/achievements?page=`
#[debug_handler]
async fn achievements(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<CharacterId>,
    Query(q): Query<PageQuery>,
) -> Result<Response> {
    respond::<Achievements>(&lodestone, &ctx, AchievementsRequest { id, page: q.page }).await
}

/// `GET /api/characters/{id}/mounts`
#[debug_handler]
async fn mounts(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<CharacterId>,
) -> Result<Response> {
    respond::<Mounts>(&lodestone, &ctx, id).await
}

/// `GET /api/characters/{id}/minions`
#[debug_handler]
async fn minions(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<CharacterId>,
) -> Result<Response> {
    respond::<Minions>(&lodestone, &ctx, id).await
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/characters")
        .add("/", get(search))
        .add("/{id}", get(show))
        .add("/{id}/class_jobs", get(class_jobs))
        .add("/{id}/achievements", get(achievements))
        .add("/{id}/mounts", get(mounts))
        .add("/{id}/minions", get(minions))
}
