use std::fmt;

/// Errors returned by the Spotify API layer.
#[derive(Debug)]
pub enum SpotifyError {
    /// The request never reached Spotify (DNS, TLS, timeout, ...).
    Transport(reqwest::Error),
    /// Spotify answered with a non-success status.
    Status { code: u16, body: String },
    /// The response body could not be decoded.
    Decode(serde_json::Error),
    /// The access token was rejected; the caller should refresh and retry.
    Unauthorized,
    /// The requested resource does not exist.
    NotFound,
    /// Spotify asked us to slow down and we gave up retrying.
    RateLimited,
}

impl SpotifyError {
    /// Whether the failure can be resolved by minting a new access token.
    pub fn is_auth_error(&self) -> bool {
        matches!(self, SpotifyError::Unauthorized)
            || matches!(self, SpotifyError::Status { code: 401, .. })
    }

    /// Whether the failure is a transient network problem worth retrying.
    pub fn is_transient(&self) -> bool {
        match self {
            SpotifyError::Transport(err) => err.is_timeout() || err.is_connect(),
            SpotifyError::Status { code, .. } => *code == 429 || *code >= 500,
            _ => false,
        }
    }

    /// HTTP status code, when the failure came from a response.
    pub fn status(&self) -> Option<u16> {
        match self {
            SpotifyError::Status { code, .. } => Some(*code),
            SpotifyError::Unauthorized => Some(401),
            SpotifyError::NotFound => Some(404),
            _ => None,
        }
    }
}

impl fmt::Display for SpotifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpotifyError::Transport(err) => write!(f, "network error: {err}"),
            SpotifyError::Status { code, body } => {
                write!(f, "Spotify returned HTTP {code}")?;
                if !body.is_empty() {
                    write!(f, ": {}", truncate(body, 300))?;
                }
                Ok(())
            }
            SpotifyError::Decode(err) => write!(f, "could not decode response: {err}"),
            SpotifyError::Unauthorized => write!(f, "access token rejected (401)"),
            SpotifyError::NotFound => write!(f, "not found (404)"),
            SpotifyError::RateLimited => write!(f, "rate limited by Spotify (429)"),
        }
    }
}

impl std::error::Error for SpotifyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SpotifyError::Transport(err) => Some(err),
            SpotifyError::Decode(err) => Some(err),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for SpotifyError {
    fn from(err: reqwest::Error) -> Self {
        SpotifyError::Transport(err)
    }
}

impl From<serde_json::Error> for SpotifyError {
    fn from(err: serde_json::Error) -> Self {
        SpotifyError::Decode(err)
    }
}

/// Shorten a string for display, collapsing newlines.
fn truncate(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let cut: String = flat.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// Convenient result alias for the Spotify layer.
pub type SpotifyResult<T> = Result<T, SpotifyError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_errors_are_flagged() {
        assert!(SpotifyError::Unauthorized.is_auth_error());
        assert!(
            SpotifyError::Status {
                code: 401,
                body: String::new()
            }
            .is_auth_error()
        );
        assert!(!SpotifyError::NotFound.is_auth_error());
    }

    #[test]
    fn transient_errors_are_flagged() {
        assert!(
            SpotifyError::Status {
                code: 429,
                body: String::new()
            }
            .is_transient()
        );
        assert!(
            SpotifyError::Status {
                code: 503,
                body: String::new()
            }
            .is_transient()
        );
        assert!(!SpotifyError::NotFound.is_transient());
    }

    #[test]
    fn status_body_is_truncated() {
        let err = SpotifyError::Status {
            code: 500,
            body: "x".repeat(1000),
        };
        let text = err.to_string();
        assert!(text.contains("HTTP 500"));
        assert!(text.len() < 400);
    }
}
