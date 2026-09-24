//! A stand-in Lodestone on `127.0.0.1:5199` (see `config/test.yaml`) that
//! serves minimal pages shaped to the vendored selectors and records every hit.

use std::sync::{Arc, Mutex};

use axum::{
    extract::{Request, State},
    http::{header::USER_AGENT, StatusCode},
    response::{Html, IntoResponse, Response},
    Router,
};

pub const CHARACTER: &str = r#"<html><body>
<div class="frame__chara">
  <a class="frame__chara__link"></a>
  <div class="frame__chara__box">
    <p class="frame__chara__title">The Tester</p>
    <p class="frame__chara__name">Test Character</p>
    <p class="frame__chara__world"><i></i>Gilgamesh&nbsp;[Aether]</p>
  </div>
</div>
<div class="character__profile__data">
  <div class="character-block"><img src="x"><div class="character-block__box">
    <p class="character-block__title">Race/Clan/Gender</p>
    <p class="character-block__name">Au Ra<br>Raen / &#9792;</p>
  </div></div>
</div>
<div class="character__freecompany__name"><p>Free Company</p>
  <h4><a href="/lodestone/freecompany/9229142273877347144/">Test &amp; Co</a></h4>
</div>
<div class="character__selfintroduction">Hello<br>World</div>
</body></html>"#;

pub const CLASS_JOBS: &str = r#"<html><body>
<div class="character__content">
  <div></div>
  <div><div><h4>Tank</h4><ul>
    <li><div><img></div><div>90</div><div>Paladin</div><div>1,234 / 5,000</div></li>
  </ul></div></div>
</div>
</body></html>"#;

pub const MOUNTS: &str = r#"<html><body>
<div class="minion__sort__total"><span>2</span></div>
<ul>
  <li class="mount__list__item"><img class="mount__list__icon__image" src="https://img.example/1.png"><span class="mount__name">Company Chocobo</span></li>
  <li class="mount__list__item"><img class="mount__list__icon__image" src="https://img.example/2.png"><span class="mount__name">Magitek Armor</span></li>
</ul>
</body></html>"#;

/// What a desktop user agent gets for mounts: no list at all.
pub const MOUNTS_DESKTOP: &str = "<html><body><p>Please use a smartphone.</p></body></html>";

pub const CHARACTER_SEARCH: &str = r#"<html><body>
<div class="ldst__window">
  <div class="entry">
    <a class="entry__link" href="/lodestone/character/1/"></a>
    <p class="entry__name">Test Character</p>
    <p class="entry__world">Gilgamesh [Aether]</p>
  </div>
  <ul class="btn__pager"><li></li><li></li><li>Page 1 of 3</li><li><a href="?page=2"></a></li></ul>
</div>
</body></html>"#;

#[derive(Debug, Clone)]
pub struct Hit {
    pub path_and_query: String,
    pub user_agent: String,
}

#[derive(Clone, Default)]
pub struct Upstream(Arc<Mutex<Vec<Hit>>>);

impl Upstream {
    /// Binds the fixed test port; the server lives as long as the test runtime.
    pub async fn start() -> Self {
        let upstream = Self::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:5199")
            .await
            .unwrap();
        let app = Router::new().fallback(serve).with_state(upstream.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        upstream
    }

    pub fn hits(&self) -> Vec<Hit> {
        self.0.lock().unwrap().clone()
    }

    pub fn count(&self, path: &str) -> usize {
        self.hits()
            .iter()
            .filter(|h| h.path_and_query.starts_with(path))
            .count()
    }
}

async fn serve(State(upstream): State<Upstream>, req: Request) -> Response {
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(ToString::to_string)
        .unwrap_or_default();
    let user_agent = req
        .headers()
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let mobile = user_agent.contains("iPhone");
    upstream.0.lock().unwrap().push(Hit {
        path_and_query,
        user_agent,
    });

    match req.uri().path() {
        "/lodestone/character/1/" | "/lodestone/character/3/" => Html(CHARACTER).into_response(),
        "/lodestone/character/1/class_job/" => Html(CLASS_JOBS).into_response(),
        "/lodestone/character/1/mount/" if mobile => Html(MOUNTS).into_response(),
        "/lodestone/character/1/mount/" => Html(MOUNTS_DESKTOP).into_response(),
        "/lodestone/character/3/class_job/" => StatusCode::FORBIDDEN.into_response(),
        "/lodestone/character/4/" => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        "/lodestone/character/" => Html(CHARACTER_SEARCH).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
