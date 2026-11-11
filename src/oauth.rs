use crate::auth::{AuthError, AuthResult};
use data_encoding::BASE64URL_NOPAD;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Spotify accounts authorization endpoint.
const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";

/// Spotify accounts token endpoint.
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// Scopes required for browsing, searching, and controlling playback.
pub const SCOPES: &[&str] = &[
    "user-read-private",
    "user-read-email",
    "user-library-read",
    "user-library-modify",
    "user-top-read",
    "playlist-read-private",
    "playlist-read-collaborative",
    "playlist-modify-private",
    "playlist-modify-public",
    "reading-playback-state",
    "modifying-playback-state",
    "app-remote-control",
];

/// The PKCE parameters for one authorization attempt.
#[derive(Debug, Clone)]
pub struct PkceSession {
    /// Verifier that is kept secret until the token request.
    pub verifier: String,
    /// Challenge derived from the verifier.
    pub challenge: String,
    /// Anti-CSRF value echoed back by Spotify.
    pub state: String,
    /// Redirect URI the browser is sent back to.
    pub redirect_uri: String,
}

impl PkceSession {
    /// Start a new authorization session.
    pub fn new(redirect_uri: impl Into<String>) -> Self {
        let verifier = random_urlsafe(64);
        let challenge = derive_challenge(&verifier);
        let state = random_urlsafe(16);
        Self {
            verifier,
            challenge,
            state,
            redirect_uri: redirect_uri.into(),
        }
    }

    /// The URL the user must open to authorize the application.
    pub fn authorize_url(&self, client_id: &str) -> String {
        let scopes = SCOPES.join(" ");
        format!(
            "{AUTHORIZE_URL}?response_type=code&client_id={client_id}&scope={scopes}\
             &redirect_uri={}&state={}&code_challenge_method=S256&code_challenge={}",
            url_encode(&self.redirect_uri),
            url_encode(&self.state),
            url_encode(&self.challenge),
        )
    }
}

/// Derive the S256 code challenge from a verifier.
pub fn derive_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    BASE64URL_NOPAD.encode(&digest)
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code(
    client_id: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> AuthResult<crate::config::TokenCache> {
    let client = reqwest::Client::new();
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("code_verifier", verifier),
        ])
        .send()
        .await?;

    parse_token_response(response).await
}

/// Refresh an access token using a refresh token.
pub async fn refresh_token(
    client_id: &str,
    refresh_token: &str,
) -> AuthResult<crate::config::TokenCache> {
    let client = reqwest::Client::new();
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ])
        .send()
        .await?;

    parse_token_response(response).await
}

/// Read a token endpoint response.
async fn parse_token_response(
    response: reqwest::Response,
) -> AuthResult<crate::config::TokenCache> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        // The error body carries a JSON description of what went wrong.
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("error_description")
                    .or_else(|| value.get("error"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or(body);
        return Err(AuthError::OAuth(detail));
    }

    let parsed: OAuthTokenResponse = serde_json::from_str(&body)?;
    let expires_at = crate::config::now_unix() + parsed.expires_in.unwrap_or(3600) as i64;

    Ok(crate::config::TokenCache {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token.unwrap_or_default(),
        expires_at,
        client_id: parsed.client_id.unwrap_or_default(),
        sp_dc_cookie: String::new(),
    })
}

/// Token endpoint response.
#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    #[serde(default)]
    client_id: Option<String>,
    /// The granted scopes, echoed by Spotify.
    #[serde(default)]
    #[allow(dead_code)]
    scope: Option<String>,
}

/// Generate a URL-safe random string of `bytes` length.
fn random_urlsafe(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::fill(&mut buffer);
    BASE64URL_NOPAD.encode(&buffer)
}

/// Percent-encode a string for use in a query parameter.
fn url_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A tiny loopback HTTP server that captures the OAuth redirect.
///
/// The browser is sent to `http://127.0.0.1:<port>/callback`, and Spotify
/// redirects there with the authorization code.
pub struct CallbackServer {
    listener: tokio::net::TcpListener,
    state: String,
}

impl CallbackServer {
    /// Bind to an ephemeral loopback port.
    pub async fn bind(state: String) -> std::io::Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self { listener, state })
    }

    /// The redirect URI this server listens on.
    pub fn redirect_uri(&self) -> String {
        let port = self
            .listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or(0);
        format!("http://127.0.0.1:{port}/callback")
    }

    /// Wait for the browser to deliver the authorization code.
    ///
    /// Returns the code, or an error when the flow was aborted.
    pub async fn wait_for_code(&self) -> AuthResult<String> {
        loop {
            let (mut socket, _) = match self.listener.accept().await {
                Ok(pair) => pair,
                Err(err) => return Err(AuthError::OAuth(err.to_string())),
            };

            let mut buffer = vec![0u8; 8192];
            let read = match tokio::io::AsyncReadExt::read(&mut socket, &mut buffer).await {
                Ok(read) => read,
                Err(_) => continue,
            };
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let Some(target) = request.split_whitespace().nth(1) else {
                continue;
            };

            let query = target.split('?').nth(1).unwrap_or_default();
            let params: Vec<(String, String)> = query
                .split('&')
                .filter_map(|pair| pair.split_once('='))
                .map(|(k, v)| (k.to_string(), url_decode(v)))
                .collect();

            let code = params
                .iter()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.clone());
            let returned_state = params
                .iter()
                .find(|(k, _)| k == "state")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let error = params
                .iter()
                .find(|(k, _)| k == "error")
                .map(|(_, v)| v.clone());

            let response_body = match (&code, &error) {
                (Some(_), None) => {
                    "<html><body><h2>Logged in</h2><p>You can close this window and return to Rustify.</p></body></html>"
                }
                _ => {
                    "<html><body><h2>Login failed</h2><p>Return to Rustify and try again.</p></body></html>"
                }
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes()).await;
            let _ = tokio::io::AsyncWriteExt::shutdown(&mut socket).await;

            if let Some(error) = error {
                return Err(AuthError::OAuth(error));
            }
            if let Some(code) = code {
                if returned_state != self.state {
                    return Err(AuthError::OAuth("state mismatch".into()));
                }
                return Ok(code);
            }
        }
    }
}

/// Percent-decode a query parameter value.
fn url_decode(value: &str) -> String {
    let text = value.replace('+', " ");
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&out).to_string()
}

/// Shared handle to a running callback server.
pub type SharedCallbackServer = Arc<CallbackServer>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_matches_rfc7636_example() {
        // RFC 7636 Appendix B: verifier "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        let challenge = derive_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn authorize_url_contains_required_parameters() {
        let session = PkceSession::new("http://127.0.0.1:1234/callback");
        let url = session.authorize_url("client123");
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=client123"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("state="));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A1234%2Fcallback"));
        assert!(url.contains("user-library-read"));
    }

    #[test]
    fn url_encoding_round_trips() {
        assert_eq!(url_encode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(url_decode("a%20b%26c%3Dd"), "a b&c=d");
        assert_eq!(url_decode("a+b"), "a b");
    }

    #[test]
    fn verifier_and_state_are_random() {
        let a = PkceSession::new("http://127.0.0.1:1/callback");
        let b = PkceSession::new("http://127.0.0.1:1/callback");
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.state, b.state);
        assert!(a.verifier.len() >= 43);
    }

    #[tokio::test]
    async fn callback_server_binds_a_loopback_port() {
        let server = CallbackServer::bind("state".into()).await.unwrap();
        assert!(server.redirect_uri().starts_with("http://127.0.0.1:"));
    }
}
