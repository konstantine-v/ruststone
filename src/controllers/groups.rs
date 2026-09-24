//! Linkshells, cross-world linkshells and PvP teams.

use axum::extract::Query;
use loco_rs::prelude::*;

use super::{respond, PageQuery};
use crate::lodestone::{
    ids::{CwlsId, LinkshellId, PvpTeamId},
    pages::{
        groups::{Cwls, GroupRequest, Linkshell, PvpTeam},
        search::{CwlsEntry, LinkshellEntry, PvpTeamEntry, Search, SearchQuery},
    },
    Lodestone,
};

/// `GET /api/linkshells?name=&world=|dc=&page=`
#[debug_handler]
async fn search_linkshells(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Query(q): Query<SearchQuery>,
) -> Result<Response> {
    respond::<Search<LinkshellEntry>>(&lodestone, &ctx, q).await
}

/// `GET /api/linkshells/{id}?page=` (page of the member list)
#[debug_handler]
async fn linkshell(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<LinkshellId>,
    Query(q): Query<PageQuery>,
) -> Result<Response> {
    respond::<Linkshell>(&lodestone, &ctx, GroupRequest { id, page: q.page }).await
}

/// `GET /api/cwls?name=&dc=&page=`
#[debug_handler]
async fn search_cwls(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Query(q): Query<SearchQuery>,
) -> Result<Response> {
    respond::<Search<CwlsEntry>>(&lodestone, &ctx, q).await
}

/// `GET /api/cwls/{id}?page=`
#[debug_handler]
async fn cwls(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<CwlsId>,
    Query(q): Query<PageQuery>,
) -> Result<Response> {
    respond::<Cwls>(&lodestone, &ctx, GroupRequest { id, page: q.page }).await
}

/// `GET /api/pvp_teams?name=&dc=&page=`
#[debug_handler]
async fn search_pvp_teams(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Query(q): Query<SearchQuery>,
) -> Result<Response> {
    respond::<Search<PvpTeamEntry>>(&lodestone, &ctx, q).await
}

/// `GET /api/pvp_teams/{id}`
#[debug_handler]
async fn pvp_team(
    lodestone: Lodestone,
    State(ctx): State<AppContext>,
    Path(id): Path<PvpTeamId>,
) -> Result<Response> {
    respond::<PvpTeam>(&lodestone, &ctx, id).await
}

pub fn linkshell_routes() -> Routes {
    Routes::new()
        .prefix("/api/linkshells")
        .add("/", get(search_linkshells))
        .add("/{id}", get(linkshell))
}

pub fn cwls_routes() -> Routes {
    Routes::new()
        .prefix("/api/cwls")
        .add("/", get(search_cwls))
        .add("/{id}", get(cwls))
}

pub fn pvp_team_routes() -> Routes {
    Routes::new()
        .prefix("/api/pvp_teams")
        .add("/", get(search_pvp_teams))
        .add("/{id}", get(pvp_team))
}
