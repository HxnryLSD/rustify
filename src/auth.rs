use crate::config::TokenCache;
use crate::totp::{self, WebPlayerSecret};
use crate::webplayer::{
    ClientToken, WebPlayerClient, WebToken, fetch_client_token, fetch_web_player_secret,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

/// User agent presented to the Spotify web player.
///
/// The web player endpoint rejects requests that do not look like a browser.
pub const WEB_PLAYER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// Endpoint that returns the Spotify web player access token.
///
/// The older `/get_access_token` path is blocked at the CDN edge and answers
/// `403 URL Blocked`; `/api/token` is the current endpoint.
const WEB_PLAYER_TOKEN_URL: &str = "https://open.spotify.com/api/token";

/// Endpoint that returns the Spotify server time in unix seconds.
///
/// The web player signs its token request with an OTP derived from the
/// server clock, so the client must use the same clock.
const SERVER_TIME_URL: &str = "https://open.spotify.com/api/server-time";

/// Error type for the authentication flows.
#[derive(Debug)]
pub enum AuthError {
    /// The request never reached Spotify.
    Network(reqwest::Error),
    /// Spotify answered with an error status.
    Status { code: u16, body: String },
    /// The response could not be decoded.
    Decode(serde_json::Error),
    /// The `sp_dc` cookie was rejected.
    InvalidCookie,
    /// The OAuth flow was aborted or the code was rejected.
    OAuth(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Network(err) => write!(f, "network error: {err}"),
            AuthError::Status { code, body } => {
                write!(f, "Spotify returned HTTP {code}")?;
                if !body.is_empty() {
                    write!(f, ": {}", body.chars().take(300).collect::<String>())?;
                }
                Ok(())
            }
            AuthError::Decode(err) => write!(f, "could not decode response: {err}"),
            AuthError::InvalidCookie => {
                write!(
                    f,
                    "the sp_dc cookie was rejected; copy a fresh one from your browser"
                )
            }
            AuthError::OAuth(msg) => write!(f, "OAuth error: {msg}"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<reqwest::Error> for AuthError {
    fn from(err: reqwest::Error) -> Self {
        AuthError::Network(err)
    }
}

impl From<serde_json::Error> for AuthError {
    fn from(err: serde_json::Error) -> Self {
        AuthError::Decode(err)
    }
}

/// Convenient result alias for the auth layer.
pub type AuthResult<T> = Result<T, AuthError>;

/// A signed-in session: the web token, the client token, and the client that
/// uses them.
pub struct Session {
    /// The client for the web-player API.
    pub client: WebPlayerClient,
    /// The web token.
    pub web_token: WebToken,
    /// The client token.
    pub client_token: ClientToken,
    /// The token cache to persist.
    pub cache: TokenCache,
}

/// Spotify authentication against the web-player token endpoint.
///
/// This flow is not sanctioned by Spotify — the endpoint's own error note
/// states that its use is "not permitted under the Spotify Developer Terms".
/// It is offered for convenience; [`crate::oauth`] implements the supported
/// Authorization Code flow with PKCE.
#[derive(Clone)]
pub struct SpotifyAuth {
    http: reqwest::Client,
    secret: Arc<WebPlayerSecret>,
}

impl Default for SpotifyAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl SpotifyAuth {
    /// Build an auth client with the bundled fallback secret.
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            secret: Arc::new(WebPlayerSecret::fallback()),
        }
    }

    /// Build an auth client that uses a secret read from the live bundle.
    pub async fn with_live_secret() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let secret = fetch_web_player_secret(&http).await;
        Self {
            http,
            secret: Arc::new(secret),
        }
    }

    /// The TOTP secret in use.
    pub fn secret(&self) -> &WebPlayerSecret {
        &self.secret
    }

    /// Fetch the Spotify server time in unix seconds.
    ///
    /// Falls back to the local clock when the endpoint cannot be reached.
    async fn server_time(&self) -> i64 {
        match self
            .http
            .get(SERVER_TIME_URL)
            .header("User-Agent", WEB_PLAYER_USER_AGENT)
            .send()
            .await
        {
            Ok(response) => match response.json::<ServerTimeResponse>().await {
                Ok(payload) => payload.server_time,
                Err(_) => totp::now_unix(),
            },
            Err(_) => totp::now_unix(),
        }
    }

    /// Sign in with the `sp_dc` cookie and return a full session.
    ///
    /// The web player signs its token request with two one-time passwords:
    /// one derived from the local clock and one from Spotify's server clock.
    pub async fn sign_in(&self, sp_dc: &str) -> AuthResult<Session> {
        let sp_dc = sp_dc.trim();
        if sp_dc.is_empty() {
            return Err(AuthError::InvalidCookie);
        }

        let server_time = self.server_time().await;
        let otp_local = self.secret.totp(totp::now_unix());
        let otp_server = self.secret.totp(server_time);

        // The web player first asks for a transport token and falls back to
        // an init token when the transport attempt is refused.
        let mut last_error: Option<AuthError> = None;
        let mut web_token = None;
        for reason in ["transport", "init"] {
            match self
                .request_web_player_token(sp_dc, reason, &otp_local, &otp_server)
                .await
            {
                Ok(token) => {
                    web_token = Some(token);
                    break;
                }
                Err(err) => last_error = Some(err),
            }
        }
        let web_token = web_token.ok_or_else(|| last_error.unwrap_or(AuthError::InvalidCookie))?;

        let client_token = fetch_client_token(&self.http, &web_token.client_id, &device_id())
            .await
            .map_err(|err| AuthError::Status {
                code: err.status().unwrap_or(0),
                body: err.to_string(),
            })?;

        let cache = TokenCache {
            access_token: web_token.access_token.clone(),
            refresh_token: String::new(),
            expires_at: web_token.expires_at,
            client_id: web_token.client_id.clone(),
            sp_dc_cookie: sp_dc.to_string(),
        };

        Ok(Session {
            client: WebPlayerClient::new(web_token.clone(), client_token.clone())
                .with_secret(self.secret.as_ref().clone()),
            web_token,
            client_token,
            cache,
        })
    }

    /// Refresh the session using a stored `sp_dc` cookie.
    pub async fn refresh(&self, sp_dc: &str) -> AuthResult<Session> {
        self.sign_in(sp_dc).await
    }

    /// Perform a single web-player token request.
    async fn request_web_player_token(
        &self,
        sp_dc: &str,
        reason: &str,
        otp_local: &str,
        otp_server: &str,
    ) -> AuthResult<WebToken> {
        let totp_ver = self.secret.version.to_string();
        let response = self
            .http
            .get(WEB_PLAYER_TOKEN_URL)
            .query(&[
                ("reason", reason),
                ("productType", "web-player"),
                ("totp", otp_local),
                ("totpServer", otp_server),
                ("totpVer", totp_ver.as_str()),
            ])
            .header("Cookie", format!("sp_dc={sp_dc}"))
            .header("User-Agent", WEB_PLAYER_USER_AGENT)
            .header("Referer", "https://open.spotify.com/")
            .header("Origin", "https://open.spotify.com")
            .header("Accept", "application/json")
            .header("App-Platform", "WebPlayer")
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(AuthError::Status {
                code: status.as_u16(),
                body,
            });
        }

        let parsed: WebPlayerTokenResponse = serde_json::from_str(&body)?;
        // Spotify answers with an anonymous token instead of an error when the
        // cookie is not accepted.
        if parsed.is_anonymous.unwrap_or(false) || parsed.access_token.is_empty() {
            return Err(AuthError::InvalidCookie);
        }

        let expires_at = parsed
            .access_token_expiration_timestamp_ms
            .map(|ms| (ms / 1000) as i64)
            .unwrap_or_else(|| totp::now_unix() + 3600);

        Ok(WebToken {
            access_token: parsed.access_token,
            client_id: parsed.client_id.unwrap_or_default(),
            expires_at,
            sp_dc: Some(sp_dc.to_string()),
        })
    }
}

/// A stable per-installation device id.
///
/// The web player sends a 36 character UUID with every request. It
/// identifies the install and carries no personal data.
pub fn device_id() -> String {
    use std::sync::OnceLock;
    static DEVICE_ID: OnceLock<String> = OnceLock::new();
    DEVICE_ID
        .get_or_init(|| {
            let mut bytes = [0u8; 16];
            rand::fill(&mut bytes);
            // Format it as a version 4 UUID.
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;

            let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
            format!(
                "{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32]
            )
        })
        .clone()
}

/// Response of the Spotify server-time endpoint.
#[derive(Debug, Deserialize)]
struct ServerTimeResponse {
    #[serde(rename = "serverTime")]
    server_time: i64,
}

/// Response of the Spotify web player token endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebPlayerTokenResponse {
    access_token: String,
    access_token_expiration_timestamp_ms: Option<u64>,
    client_id: Option<String>,
    #[serde(default)]
    is_anonymous: Option<bool>,
}

/// Thread-safe authentication state shared across the app.
#[derive(Clone, Default)]
pub struct AuthState {
    inner: Arc<Mutex<AuthStateInner>>,
}

#[derive(Default)]
pub struct AuthStateInner {
    pub tokens: Option<TokenCache>,
    pub is_authenticated: bool,
    pub last_error: Option<String>,
}

impl AuthState {
    /// Create an empty auth state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a freshly minted token set.
    pub fn set_tokens(&self, tokens: TokenCache) {
        let mut inner = self.inner.lock().unwrap();
        inner.tokens = Some(tokens);
        inner.is_authenticated = true;
        inner.last_error = None;
    }

    /// Read the current token set.
    pub fn get_tokens(&self) -> Option<TokenCache> {
        self.inner.lock().unwrap().tokens.clone()
    }

    /// Whether a token set is present.
    pub fn is_authenticated(&self) -> bool {
        self.inner.lock().unwrap().is_authenticated
    }

    /// Record an error message.
    pub fn set_error(&self, error: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.last_error = Some(error);
        inner.is_authenticated = false;
    }

    /// Read and clear the last error.
    pub fn take_error(&self) -> Option<String> {
        let mut inner = self.inner.lock().unwrap();
        inner.last_error.take()
    }

    /// Forget the current session.
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.tokens = None;
        inner.is_authenticated = false;
        inner.last_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_cookie() {
        let auth = SpotifyAuth::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(auth.sign_in("   \t "));
        assert!(matches!(result, Err(AuthError::InvalidCookie)));
    }

    #[test]
    fn auth_state_tracks_session() {
        let state = AuthState::new();
        assert!(!state.is_authenticated());
        assert!(state.get_tokens().is_none());

        state.set_tokens(TokenCache {
            access_token: "tok".into(),
            expires_at: 42,
            ..Default::default()
        });
        assert!(state.is_authenticated());
        assert_eq!(state.get_tokens().unwrap().access_token, "tok");

        state.set_error("boom".into());
        assert_eq!(state.take_error().unwrap(), "boom");
        assert!(state.take_error().is_none());

        state.clear();
        assert!(!state.is_authenticated());
    }

    #[test]
    fn server_time_response_is_parsed() {
        let parsed: ServerTimeResponse =
            serde_json::from_str(r#"{"serverTime":1794425384}"#).unwrap();
        assert_eq!(parsed.server_time, 1_794_425_384);
    }

    #[test]
    fn anonymous_token_response_is_detected() {
        let parsed: WebPlayerTokenResponse =
            serde_json::from_str(r#"{"accessToken":"x","isAnonymous":true}"#).unwrap();
        assert_eq!(parsed.is_anonymous, Some(true));
    }

    #[test]
    fn the_device_id_is_stable_and_well_formed() {
        assert_eq!(device_id(), device_id());
        let id = device_id();
        assert_eq!(id.len(), 36, "the device id must be 36 characters");
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "the device id must be hexadecimal"
        );
    }
}
