//! Lodestone client: fetch → cache → parse, generic over [`Page`].
//!
//! Adding a page: vendor its selector file, add a field to
//! [`selectors::Selectors`], write a `pages/<name>.rs` that implements
//! [`Page`], and expose it from a controller.

pub mod error;
pub mod ids;
pub mod pages;
pub mod selector;
pub mod selectors;

use std::{
    fmt,
    marker::PhantomData,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{extract::FromRequestParts, http::request::Parts};
use loco_rs::{app::AppContext, cache::Cache};
use reqwest::{header::USER_AGENT, StatusCode};
use scraper::Html;
use serde::{Serialize, Serializer};
use serde_json::value::RawValue;
use tokio::sync::Semaphore;
use url::Url;

pub use self::{
    error::{LodestoneError, ParseError},
    pages::search::SearchTarget,
    selectors::Selectors,
};
use crate::settings::Settings;

/// What a page is. Drives the cache key prefix, the TTL and the user agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Character,
    ClassJobs,
    Achievements,
    Mounts,
    Minions,
    FreeCompany,
    FreeCompanyMembers,
    Linkshell,
    CrossworldLinkshell,
    PvpTeam,
    Search(SearchTarget),
}

impl PageKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::ClassJobs => "class_jobs",
            Self::Achievements => "achievements",
            Self::Mounts => "mounts",
            Self::Minions => "minions",
            Self::FreeCompany => "free_company",
            Self::FreeCompanyMembers => "free_company_members",
            Self::Linkshell => "linkshell",
            Self::CrossworldLinkshell => "cwls",
            Self::PvpTeam => "pvp_team",
            Self::Search(target) => target.as_str(),
        }
    }

    /// Lodestone only renders these lists for mobile user agents.
    const fn wants_mobile(self) -> bool {
        matches!(self, Self::Mounts | Self::Minions)
    }
}

/// One kind of Lodestone page: where it lives and how to read it.
pub trait Page: Serialize + Sized + Send + 'static {
    const KIND: PageKind;
    /// Everything that identifies one page; also serialised into the cache key.
    type Request: Serialize + Send + 'static;

    fn url(base: &Url, req: &Self::Request) -> Url;

    /// # Errors
    /// When a field the page cannot exist without is missing.
    fn parse(doc: &Html, sel: &Selectors, req: &Self::Request) -> Result<Self, ParseError>;
}

/// The JSON of a parsed `P`, exactly as cached and served.
///
/// A cache hit is written straight to the response without being
/// deserialised. The type parameter keeps responses typed: a `Cached<P>` only
/// comes from [`Lodestone::fetch::<P>`].
pub struct Cached<P> {
    raw: Box<RawValue>,
    _page: PhantomData<fn() -> P>,
}

impl<P> Cached<P> {
    const fn new(raw: Box<RawValue>) -> Self {
        Self {
            raw,
            _page: PhantomData,
        }
    }

    #[must_use]
    pub fn json(&self) -> &str {
        self.raw.get()
    }
}

impl<P> fmt::Debug for Cached<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Cached").field(&self.raw.get()).finish()
    }
}

impl<P> Serialize for Cached<P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}

/// Shared Lodestone client. Cheap to clone; lives in `ctx.shared_store` and
/// is available to handlers as an extractor.
#[derive(Clone)]
pub struct Lodestone(Arc<Inner>);

struct Inner {
    http: reqwest::Client,
    selectors: Selectors,
    settings: Settings,
    base: Url,
    limiter: Semaphore,
}

impl Lodestone {
    /// # Errors
    /// When the base URL is invalid or the HTTP client cannot be built.
    pub fn new(settings: Settings, selectors: Selectors) -> loco_rs::Result<Self> {
        // Errors only if a provider is already installed, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .timeout(settings.lodestone.timeout())
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .map_err(|e| loco_rs::Error::Message(format!("http client: {e}")))?;
        Ok(Self(Arc::new(Inner {
            http,
            selectors,
            base: settings.lodestone.base_url()?,
            limiter: Semaphore::new(settings.lodestone.max_concurrent.max(1)),
            settings,
        })))
    }

    /// Cache lifetime for a page kind (also used for `Cache-Control`).
    #[must_use]
    pub fn ttl(&self, kind: PageKind) -> Duration {
        self.0.settings.cache_ttl_secs.for_kind(kind)
    }

    /// Returns page `P` for `req` from the cache, or fetches, parses and
    /// caches it. Cache failures are logged and never fail the request; only
    /// successful parses are cached.
    ///
    /// # Errors
    /// See [`LodestoneError`].
    pub async fn fetch<P: Page>(
        &self,
        cache: &Cache,
        req: P::Request,
    ) -> Result<Cached<P>, LodestoneError> {
        let ttl = self.ttl(P::KIND);
        let key = if ttl.is_zero() {
            None
        } else {
            self.cache_key::<P>(&req)
        };

        if let Some(key) = &key {
            match cache.get::<Box<RawValue>>(key).await {
                Ok(Some(raw)) => return Ok(Cached::new(raw)),
                Ok(None) => {}
                Err(err) => tracing::warn!(%err, key, "cache read failed"),
            }
        }

        let body = self.get_html(P::KIND, P::url(&self.0.base, &req)).await?;
        let this = self.clone();
        let raw = tokio::task::spawn_blocking(move || {
            let doc = Html::parse_document(&body);
            let page = P::parse(&doc, &this.0.selectors, &req)?;
            serde_json::value::to_raw_value(&page).map_err(|e| ParseError::Task(e.to_string()))
        })
        .await
        .map_err(|e| ParseError::Task(e.to_string()))??;

        if let Some(key) = &key {
            if let Err(err) = cache.insert_with_expiry(key, &*raw, ttl).await {
                tracing::warn!(%err, key, "cache write failed");
            }
        }
        Ok(Cached::new(raw))
    }

    fn cache_key<P: Page>(&self, req: &P::Request) -> Option<String> {
        let req = serde_json::to_string(req).ok()?;
        let region = self.0.settings.lodestone.region.subdomain();
        Some(format!("lodestone:{}:{region}:{req}", P::KIND.as_str()))
    }

    async fn get_html(&self, kind: PageKind, url: Url) -> Result<String, LodestoneError> {
        // The semaphore is never closed, so `acquire` cannot fail.
        let _permit = self.0.limiter.acquire().await.ok();
        let lodestone = &self.0.settings.lodestone;
        let user_agent = if kind.wants_mobile() {
            &lodestone.user_agent_mobile
        } else {
            &lodestone.user_agent_desktop
        };

        let started = Instant::now();
        let response = self
            .0
            .http
            .get(url.clone())
            .header(USER_AGENT, user_agent)
            .send()
            .await?;
        let status = response.status();
        tracing::debug!(%url, %status, elapsed_ms = started.elapsed().as_millis(), "lodestone fetch");

        match status {
            s if s.is_success() => Ok(response.text().await?),
            StatusCode::NOT_FOUND => Err(LodestoneError::NotFound),
            StatusCode::FORBIDDEN => Err(LodestoneError::Private),
            StatusCode::TOO_MANY_REQUESTS => Err(LodestoneError::RateLimited),
            StatusCode::SERVICE_UNAVAILABLE => Err(LodestoneError::Maintenance),
            s => Err(LodestoneError::Upstream(s)),
        }
    }
}

impl FromRequestParts<AppContext> for Lodestone {
    type Rejection = loco_rs::Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        ctx: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        ctx.shared_store
            .get::<Self>()
            .ok_or_else(|| loco_rs::Error::Message("Lodestone client is not initialised".into()))
    }
}

/// `base` + percent-encoded `segments` + trailing `/`.
pub(crate) fn page_url<'a>(base: &Url, segments: impl IntoIterator<Item = &'a str>) -> Url {
    let mut url = base.clone();
    // `Settings::base_url` guarantees a base URL, so this cannot fail.
    if let Ok(mut path) = url.path_segments_mut() {
        path.pop_if_empty().extend(segments).push("");
    }
    url
}
