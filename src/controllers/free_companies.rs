use axum::extract::Query;
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};

use super::{cached_json, include, respond, Included, PageQuery};
use crate::lodestone::{
    ids::{CommaList, FreeCompanyId, PageNo},
    pages::{
        free_company::{FreeCompany, FreeCompanyMembers, MembersRequest},
        search::{FreeCompanyEntry, Search, SearchQuery},
    },
    Cached, Lodestone, Page,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Members,
}

#[derive(Debug, Deserialize)]
pub struct ShowQuery {
    #[serde(default)]
    pub include: CommaList<Section>,
}

#[derive(Serialize)]
struct FreeCompanyResponse {
    free_company: Cached<FreeCompany>,
    #[serde(skip_serializing_if = "Included::is_not_requested")]
    members: Included<Cached<FreeCompanyMembers>>,
}

/// `GET /api/free_companies?name=&world=|dc=&page=`
#[debug_handler]
async fn search(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Query(q): Query<SearchQuery>,
) -> Result<Response> {
    respond::<Search<FreeCompanyEntry>>(&lodestone, &ctx, q).await
}

/// `GET /api/free_companies/{id}?include=members` (members: first page).
#[debug_handler]
async fn show(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<FreeCompanyId>,
    Query(q): Query<ShowQuery>,
) -> Result<Response> {
    let cache = &ctx.cache;
    let wants_members = q.include.contains(&Section::Members);
    let (free_company, members) = tokio::try_join!(
        lodestone.fetch::<FreeCompany>(cache, id),
        include(
            wants_members,
            lodestone.fetch::<FreeCompanyMembers>(
                cache,
                MembersRequest {
                    id,
                    page: PageNo::FIRST
                }
            ),
        ),
    )?;

    let mut max_age = lodestone.ttl(FreeCompany::KIND);
    if wants_members {
        max_age = max_age.min(lodestone.ttl(FreeCompanyMembers::KIND));
    }
    cached_json(
        max_age,
        FreeCompanyResponse {
            free_company,
            members,
        },
    )
}

/// `GET /api/free_companies/{id}/members?page=`
#[debug_handler]
async fn members(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<FreeCompanyId>,
    Query(q): Query<PageQuery>,
) -> Result<Response> {
    respond::<FreeCompanyMembers>(&lodestone, &ctx, MembersRequest { id, page: q.page }).await
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/free_companies")
        .add("/", get(search))
        .add("/{id}", get(show))
        .add("/{id}/members", get(members))
}
