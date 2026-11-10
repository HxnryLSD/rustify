use serde::Deserialize;
use std::sync::Arc;

use crate::config::TokenCache;

/// Spotify authentication using the sp_dc browser cookie.
///
/// No Spotify Developer Dashboard registration, no client_id, no client_secret.
/// The user provides their sp_dc cookie value (from their browser's Spotify cookies),
/// and Rustify uses it to obtain an access token from Spotify's token endpoint.
pub struct SpotifyAuth {
    sp_dc_cookie: String,
    http_client: reqwest::Client,
}

impl SpotifyAuth {
    pub fn new(sp_dc_cookie: &str) -> Self {
        Self {
            sp_dc_cookie: sp_dc_cookie.to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Exchange the sp_dc cookie for an access token via Spotify's token endpoint.
    pub async fn exchange_cookie_for_token(
        &self,
    ) -> Result<TokenCache, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .http_client
            .post("https://open.spotify.com/get_access_token")
            .header("Cookie", format!("sp_dc={}", self.sp_dc_cookie))
            .header("Referer", "https://open.spotify.com/")
            .header("Origin", "https://open.spotify.com")
            .header(
                "User-Agent",
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
            )
            .send()
            .await?
            .error_for_status()?;

        let token_response: SpotifyTokenResponse = resp.json().await?;
        Ok(token_response.into_token_cache())
    }

    /// Refresh an expired access token using the sp_dc cookie again.
    pub async fn refresh_with_cookie(
        &self,
    ) -> Result<TokenCache, Box<dyn std::error::Error + Send + Sync>> {
        self.exchange_cookie_for_token().await
    }
}

/// Raw Spotify token response from the get_access_token endpoint.
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct SpotifyTokenResponse {
    accessToken: String,
    accessTokenExpirationTimestampMs: u64,
}

impl SpotifyTokenResponse {
    fn into_token_cache(self) -> TokenCache {
        TokenCache {
            access_token: self.accessToken,
            refresh_token: String::new(), // sp_dc flow does not issue refresh tokens
            expires_at: (self.accessTokenExpirationTimestampMs / 1000) as i64,
        }
    }
}

/// Thread-safe authentication state shared across the app.
#[derive(Clone, Default)]
pub struct AuthState {
    inner: Arc<std::sync::Mutex<AuthStateInner>>,
}

#[derive(Default)]
pub struct AuthStateInner {
    pub tokens: Option<TokenCache>,
    pub is_authenticated: bool,
    pub last_error: Option<String>,
}

impl AuthState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_tokens_blocking(&self, tokens: TokenCache) {
        let mut inner = self.inner.lock().unwrap();
        inner.tokens = Some(tokens);
        inner.is_authenticated = true;
    }

    pub fn get_tokens_blocking(&self) -> Option<TokenCache> {
        self.inner.lock().unwrap().tokens.clone()
    }

    pub async fn set_tokens(&self, tokens: TokenCache) {
        let mut inner = self.inner.lock().unwrap();
        inner.tokens = Some(tokens);
        inner.is_authenticated = true;
    }

    pub async fn get_tokens(&self) -> Option<TokenCache> {
        self.inner.lock().unwrap().tokens.clone()
    }

    pub fn is_authenticated_blocking(&self) -> bool {
        self.inner.lock().unwrap().is_authenticated
    }

    pub fn set_error_blocking(&self, error: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.last_error = Some(error);
    }

    pub fn get_error_blocking(&self) -> Option<String> {
        self.inner.lock().unwrap().last_error.clone()
    }

    pub fn clear_error_blocking(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.last_error = None;
    }

    pub async fn is_authenticated(&self) -> bool {
        self.inner.lock().unwrap().is_authenticated
    }

    pub async fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.tokens = None;
        inner.is_authenticated = false;
    }

    pub fn clear_blocking(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.tokens = None;
        inner.is_authenticated = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_response_converts_to_cache() {
        let resp = SpotifyTokenResponse {
            accessToken: "test_token".to_string(),
            accessTokenExpirationTimestampMs: 9999999999000,
        };
        let cache = resp.into_token_cache();
        assert_eq!(cache.access_token, "test_token");
        assert_eq!(cache.refresh_token, "");
        assert_eq!(cache.expires_at, 9999999999);
    }
}
