use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::TokenCache;

/// Spotify OAuth 2.0 client configuration.
pub struct SpotifyAuth {
    client_id: String,
    client_secret: String,
    redirect_url: String,
    http_client: reqwest::Client,
}

impl SpotifyAuth {
    pub fn new(client_id: &str, client_secret: &str, redirect_url: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_url: redirect_url.to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Generate the authorization URL the user should visit in a browser.
    pub fn authorize_url(&self) -> (String, CsrfToken) {
        let auth_url = AuthUrl::new("https://accounts.spotify.com/authorize".to_string()).unwrap();
        let token_url =
            TokenUrl::new("https://accounts.spotify.com/api/token".to_string()).unwrap();

        let client = oauth2::basic::BasicClient::new(ClientId::new(self.client_id.clone()))
            .set_client_secret(ClientSecret::new(self.client_secret.clone()))
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(RedirectUrl::new(self.redirect_url.clone()).unwrap());

        let (auth_url, csrf_state) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("user-read-private".to_string()))
            .add_scope(Scope::new("user-read-email".to_string()))
            .add_scope(Scope::new("user-library-read".to_string()))
            .add_scope(Scope::new("user-library-modify".to_string()))
            .add_scope(Scope::new("playlist-read-private".to_string()))
            .add_scope(Scope::new("playlist-modify-private".to_string()))
            .add_scope(Scope::new("user-read-playback-state".to_string()))
            .add_scope(Scope::new("user-modify-playback-state".to_string()))
            .add_scope(Scope::new("user-read-currently-playing".to_string()))
            .add_scope(Scope::new("streaming".to_string()))
            .url();
        (auth_url.to_string(), csrf_state)
    }

    /// Exchange an authorization code for tokens.
    pub async fn exchange_code(
        &self,
        code: &str,
    ) -> Result<TokenCache, Box<dyn std::error::Error + Send + Sync>> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.redirect_url),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
        ];

        let resp = self
            .http_client
            .post("https://accounts.spotify.com/api/token")
            .form(&params)
            .send()
            .await?
            .error_for_status()?;

        let token_response: TokenResponse = resp.json().await?;
        Ok(token_response.into_token_cache())
    }

    /// Refresh an expired access token using the refresh token.
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<TokenCache, Box<dyn std::error::Error + Send + Sync>> {
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
        ];

        let resp = self
            .http_client
            .post("https://accounts.spotify.com/api/token")
            .form(&params)
            .send()
            .await?
            .error_for_status()?;

        let token_response: TokenResponse = resp.json().await?;
        Ok(token_response.into_token_cache())
    }
}

/// Raw Spotify token response.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
    scope: Option<String>,
}

impl TokenResponse {
    fn into_token_cache(self) -> TokenCache {
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + self.expires_in as i64;

        TokenCache {
            access_token: self.access_token,
            refresh_token: self.refresh_token.unwrap_or_default(),
            expires_at,
        }
    }
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
}

impl AuthState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_tokens(&self, tokens: TokenCache) {
        let mut inner = self.inner.lock().await;
        inner.tokens = Some(tokens);
        inner.is_authenticated = true;
    }

    pub fn set_tokens_blocking(&self, tokens: TokenCache) {
        let mut inner = self.inner.try_lock().unwrap();
        inner.tokens = Some(tokens);
        inner.is_authenticated = true;
    }

    pub async fn get_tokens(&self) -> Option<TokenCache> {
        self.inner.lock().await.tokens.clone()
    }

    pub fn get_tokens_blocking(&self) -> Option<TokenCache> {
        self.inner.try_lock().unwrap().tokens.clone()
    }

    pub async fn is_authenticated(&self) -> bool {
        self.inner.lock().await.is_authenticated
    }

    pub async fn clear(&self) {
        let mut inner = self.inner.lock().await;
        inner.tokens = None;
        inner.is_authenticated = false;
    }
}
