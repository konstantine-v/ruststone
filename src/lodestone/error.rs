use axum::http::StatusCode;
use loco_rs::controller::ErrorDetail;

/// Everything that can go wrong between a validated request and a parsed page.
#[derive(Debug, thiserror::Error)]
pub enum LodestoneError {
    #[error("not found on the Lodestone")]
    NotFound,
    #[error("this data is private on the Lodestone")]
    Private,
    #[error("the Lodestone is rate limiting requests")]
    RateLimited,
    #[error("the Lodestone is under maintenance")]
    Maintenance,
    #[error("the Lodestone did not respond in time")]
    Timeout,
    #[error("the Lodestone answered with {0}")]
    Upstream(StatusCode),
    #[error("could not reach the Lodestone: {0}")]
    Transport(#[source] reqwest::Error),
    #[error("could not parse the Lodestone page: {0}")]
    Parse(#[from] ParseError),
}

/// The page came back but did not look like what its selectors expect.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("required field `{0}` is missing")]
    Missing(&'static str),
    #[error("parser task failed: {0}")]
    Task(String),
}

impl LodestoneError {
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Private => StatusCode::FORBIDDEN,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::Maintenance => StatusCode::SERVICE_UNAVAILABLE,
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
            Self::Upstream(_) | Self::Transport(_) => StatusCode::BAD_GATEWAY,
            Self::Parse(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    const fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Private => "private",
            Self::RateLimited => "rate_limited",
            Self::Maintenance => "maintenance",
            Self::Timeout => "upstream_timeout",
            Self::Upstream(_) | Self::Transport(_) => "upstream_error",
            Self::Parse(_) => "parse_error",
        }
    }
}

impl From<reqwest::Error> for LodestoneError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout
        } else {
            Self::Transport(err)
        }
    }
}

impl From<LodestoneError> for loco_rs::Error {
    fn from(err: LodestoneError) -> Self {
        let status = err.status();
        if status.is_server_error() {
            tracing::warn!(error = %err, "lodestone request failed");
        }
        Self::CustomError(status, ErrorDetail::new(err.code(), err.to_string()))
    }
}
