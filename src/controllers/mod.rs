//! HTTP API. Handlers only extract validated input, call
//! [`Lodestone::fetch`], and respond.

pub mod characters;
pub mod free_companies;
pub mod groups;

use std::{future::Future, time::Duration};

use axum::http::header;
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::lodestone::{ids::PageNo, Lodestone, LodestoneError, Page};

/// `?page=` for paginated sub-resources (defaults to 1).
#[derive(Debug, Deserialize)]
pub struct PageQuery {
    #[serde(default)]
    pub page: PageNo,
}

/// JSON with a `Cache-Control` matching how long the data is cached here.
pub(crate) fn cached_json<T: serde::Serialize>(max_age: Duration, body: T) -> Result<Response> {
    format::render()
        .header(
            header::CACHE_CONTROL,
            format!("public, max-age={}", max_age.as_secs()),
        )
        .json(body)
}

/// Fetches one page and responds with it.
pub(crate) async fn respond<P: Page>(
    lodestone: &Lodestone,
    ctx: &AppContext,
    req: P::Request,
) -> Result<Response> {
    let body = lodestone.fetch::<P>(&ctx.cache, req).await?;
    cached_json(lodestone.ttl(P::KIND), body)
}

/// An `?include=`d section of a response.
#[derive(Debug)]
pub enum Included<T> {
    /// Not asked for: the key is left out of the response.
    NotRequested,
    /// The Lodestone reports the section as private: the key is `null`.
    Private,
    Data(T),
}

impl<T> Included<T> {
    pub const fn is_not_requested(&self) -> bool {
        matches!(self, Self::NotRequested)
    }
}

impl<T: serde::Serialize> serde::Serialize for Included<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Data(data) => data.serialize(serializer),
            Self::NotRequested | Self::Private => serializer.serialize_none(),
        }
    }
}

/// Runs `fetch` only when the section was requested.
pub(crate) async fn include<T>(
    wanted: bool,
    fetch: impl Future<Output = Result<T, LodestoneError>>,
) -> Result<Included<T>, LodestoneError> {
    if !wanted {
        return Ok(Included::NotRequested);
    }
    match fetch.await {
        Ok(data) => Ok(Included::Data(data)),
        Err(LodestoneError::Private) => Ok(Included::Private),
        Err(e) => Err(e),
    }
}
