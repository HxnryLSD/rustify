use crate::error::{SpotifyError, SpotifyResult};
use crate::models::*;
use crate::privacy::Privacy;
use reqwest::header::HeaderMap;
use reqwest::{Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Maximum number of retries for a rate-limited or transient failure.
const MAX_RETRIES: u32 = 3;

/// Base delay for the exponential backoff between retries.
const BASE_BACKOFF: Duration = Duration::from_secs(1);

/// Spotify Web API client.
///
/// Owns a single [`reqwest::Client`] so that connections are pooled and
/// reused across every request.
#[derive(Clone)]
pub struct SpotifyClient {
    client: Arc<reqwest::Client>,
    base_url: String,
}

impl Default for SpotifyClient {
    /// Build a client, falling back to a default-constructed HTTP client.
    ///
    /// A TLS backend failure is not recoverable here, so the fallback keeps
    /// the application usable instead of panicking.
    fn default() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            client: Arc::new(client),
            base_url: "https://api.spotify.com/v1".to_string(),
        }
    }
}

impl SpotifyClient {
    /// Build a client with sensible timeouts.
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(concat!("Rustify/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client: Arc::new(client),
            base_url: "https://api.spotify.com/v1".to_string(),
        })
    }

    /// Build a client that talks to a different base URL.
    pub fn with_base_url(
        base_url: impl Into<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut client = Self::new()?;
        client.base_url = base_url.into();
        Ok(client)
    }

    /// The underlying HTTP client.
    pub fn http(&self) -> &reqwest::Client {
        &self.client
    }

    /// Build a request with auth and privacy headers applied.
    fn request(
        &self,
        method: Method,
        path: &str,
        access_token: &str,
        privacy: &Privacy,
    ) -> reqwest::RequestBuilder {
        let mut headers = privacy.request_headers(access_token);
        crate::privacy::strip_telemetry(&mut headers);
        self.client
            .request(method, format!("{}{path}", self.base_url))
            .headers(headers)
    }

    /// Send a request, retrying on rate limits and transient server errors.
    async fn send(&self, request: reqwest::RequestBuilder) -> SpotifyResult<Response> {
        let mut attempt = 0;
        loop {
            let response = request
                .try_clone()
                .ok_or_else(|| SpotifyError::Status {
                    code: 0,
                    body: "request could not be cloned for retry".into(),
                })?
                .send()
                .await?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                if attempt >= MAX_RETRIES {
                    return Err(SpotifyError::RateLimited);
                }
                let wait =
                    retry_after(response.headers()).unwrap_or(BASE_BACKOFF * 2u32.pow(attempt));
                tokio::time::sleep(wait).await;
                attempt += 1;
                continue;
            }

            if response.status().is_server_error() && attempt < MAX_RETRIES {
                tokio::time::sleep(BASE_BACKOFF * 2u32.pow(attempt)).await;
                attempt += 1;
                continue;
            }

            return Ok(response);
        }
    }

    /// Send a request and decode the JSON body.
    async fn send_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> SpotifyResult<T> {
        let response = self.send(request).await?;
        let response = check_status(response).await?;
        Ok(response.json().await?)
    }

    /// Send a request that has no meaningful response body.
    async fn send_empty(&self, request: reqwest::RequestBuilder) -> SpotifyResult<()> {
        let response = self.send(request).await?;
        check_status(response).await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Profile
    // -----------------------------------------------------------------------

    /// GET current user profile.
    pub async fn get_me(
        &self,
        access_token: &str,
        privacy: &Privacy,
    ) -> SpotifyResult<CurrentUser> {
        let request = self.request(Method::GET, "/me", access_token, privacy);
        self.send_json(request).await
    }

    // -----------------------------------------------------------------------
    // Playback
    // -----------------------------------------------------------------------

    /// GET current playback state.
    pub async fn get_playback_state(
        &self,
        access_token: &str,
        privacy: &Privacy,
    ) -> SpotifyResult<Option<PlaybackState>> {
        let request = self.request(Method::GET, "/me/player", access_token, privacy);
        let response = self.send(request).await?;
        if response.status() == StatusCode::NO_CONTENT {
            return Ok(None);
        }
        let response = check_status(response).await?;
        Ok(Some(response.json().await?))
    }

    /// GET the devices available for playback.
    pub async fn get_devices(
        &self,
        access_token: &str,
        privacy: &Privacy,
    ) -> SpotifyResult<Vec<Device>> {
        let request = self.request(Method::GET, "/me/player/devices", access_token, privacy);
        let response: Value = self.send_json(request).await?;
        Ok(response
            .get("devices")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default())
    }

    /// PUT start or resume playback.
    ///
    /// Either a set of track URIs or a context URI may be supplied.
    pub async fn play(
        &self,
        access_token: &str,
        privacy: &Privacy,
        uris: Option<&[String]>,
        context_uri: Option<&str>,
        offset: Option<u32>,
        position_ms: Option<u32>,
    ) -> SpotifyResult<()> {
        let mut body = serde_json::Map::new();
        if let Some(uris) = uris
            && !uris.is_empty()
        {
            body.insert("uris".into(), serde_json::json!(uris));
        }
        if let Some(context_uri) = context_uri {
            body.insert("context_uri".into(), Value::String(context_uri.into()));
        }
        if let Some(offset) = offset {
            body.insert("offset".into(), serde_json::json!({ "position": offset }));
        }
        if let Some(position_ms) = position_ms {
            body.insert("position_ms".into(), Value::from(position_ms));
        }

        let mut request = self.request(Method::PUT, "/me/player/play", access_token, privacy);
        if !body.is_empty() {
            request = request.json(&Value::Object(body));
        }
        self.send_empty(request).await
    }

    /// PUT pause playback.
    pub async fn pause(&self, access_token: &str, privacy: &Privacy) -> SpotifyResult<()> {
        let request = self.request(Method::PUT, "/me/player/pause", access_token, privacy);
        self.send_empty(request).await
    }

    /// POST skip to the next track.
    pub async fn next_track(&self, access_token: &str, privacy: &Privacy) -> SpotifyResult<()> {
        let request = self.request(Method::POST, "/me/player/next", access_token, privacy);
        self.send_empty(request).await
    }

    /// POST skip to the previous track.
    pub async fn previous_track(&self, access_token: &str, privacy: &Privacy) -> SpotifyResult<()> {
        let request = self.request(Method::POST, "/me/player/previous", access_token, privacy);
        self.send_empty(request).await
    }

    /// PUT seek to a position in the current track.
    pub async fn seek(
        &self,
        access_token: &str,
        privacy: &Privacy,
        position_ms: u32,
    ) -> SpotifyResult<()> {
        let request = self
            .request(Method::PUT, "/me/player/seek", access_token, privacy)
            .query(&[("position_ms", position_ms.to_string())]);
        self.send_empty(request).await
    }

    /// PUT set the playback volume, 0-100.
    pub async fn set_volume(
        &self,
        access_token: &str,
        privacy: &Privacy,
        percent: u32,
    ) -> SpotifyResult<()> {
        let percent = percent.min(100);
        let request = self
            .request(Method::PUT, "/me/player/volume", access_token, privacy)
            .query(&[("volume_percent", percent.to_string())]);
        self.send_empty(request).await
    }

    /// PUT toggle shuffle.
    pub async fn set_shuffle(
        &self,
        access_token: &str,
        privacy: &Privacy,
        state: bool,
    ) -> SpotifyResult<()> {
        let request = self
            .request(Method::PUT, "/me/player/shuffle", access_token, privacy)
            .query(&[("state", state.to_string())]);
        self.send_empty(request).await
    }

    /// PUT set the repeat mode (`off`, `track`, `context`).
    pub async fn set_repeat(
        &self,
        access_token: &str,
        privacy: &Privacy,
        state: &str,
    ) -> SpotifyResult<()> {
        let request = self
            .request(Method::PUT, "/me/player/repeat", access_token, privacy)
            .query(&[("state", state)]);
        self.send_empty(request).await
    }

    /// PUT transfer playback to another device.
    pub async fn transfer_playback(
        &self,
        access_token: &str,
        privacy: &Privacy,
        device_ids: &[String],
        play: bool,
    ) -> SpotifyResult<()> {
        let request = self
            .request(Method::PUT, "/me/player", access_token, privacy)
            .json(&serde_json::json!({
                "device_ids": device_ids,
                "play": play,
            }));
        self.send_empty(request).await
    }

    // -----------------------------------------------------------------------
    // Browse and search
    // -----------------------------------------------------------------------

    /// GET search tracks, albums, artists, and playlists.
    pub async fn search(
        &self,
        access_token: &str,
        privacy: &Privacy,
        query: &str,
        types: &str,
        limit: u32,
    ) -> SpotifyResult<SearchResponse> {
        let request = self
            .request(Method::GET, "/search", access_token, privacy)
            .query(&[
                ("q", query),
                ("type", types),
                ("limit", limit.to_string().as_str()),
            ]);
        self.send_json(request).await
    }

    /// GET track details.
    pub async fn get_track(
        &self,
        access_token: &str,
        privacy: &Privacy,
        track_id: &str,
    ) -> SpotifyResult<Track> {
        let request = self.request(
            Method::GET,
            &format!("/tracks/{track_id}"),
            access_token,
            privacy,
        );
        self.send_json(request).await
    }

    /// GET album details.
    pub async fn get_album(
        &self,
        access_token: &str,
        privacy: &Privacy,
        album_id: &str,
    ) -> SpotifyResult<Album> {
        let request = self.request(
            Method::GET,
            &format!("/albums/{album_id}"),
            access_token,
            privacy,
        );
        self.send_json(request).await
    }

    /// GET the tracks of an album.
    pub async fn get_album_tracks(
        &self,
        access_token: &str,
        privacy: &Privacy,
        album_id: &str,
        limit: u32,
    ) -> SpotifyResult<Paging<Track>> {
        let request = self
            .request(
                Method::GET,
                &format!("/albums/{album_id}/tracks"),
                access_token,
                privacy,
            )
            .query(&[("limit", limit.to_string().as_str())]);
        self.send_json(request).await
    }

    /// GET artist details.
    pub async fn get_artist(
        &self,
        access_token: &str,
        privacy: &Privacy,
        artist_id: &str,
    ) -> SpotifyResult<Artist> {
        let request = self.request(
            Method::GET,
            &format!("/artists/{artist_id}"),
            access_token,
            privacy,
        );
        self.send_json(request).await
    }

    /// GET an artist's top tracks.
    pub async fn get_artist_top_tracks(
        &self,
        access_token: &str,
        privacy: &Privacy,
        artist_id: &str,
        market: &str,
    ) -> SpotifyResult<Paging<Track>> {
        let request = self
            .request(
                Method::GET,
                &format!("/artists/{artist_id}/top-tracks"),
                access_token,
                privacy,
            )
            .query(&[("market", market)]);
        self.send_json(request).await
    }

    /// GET an artist's albums.
    pub async fn get_artist_albums(
        &self,
        access_token: &str,
        privacy: &Privacy,
        artist_id: &str,
        limit: u32,
    ) -> SpotifyResult<Paging<Album>> {
        let request = self
            .request(
                Method::GET,
                &format!("/artists/{artist_id}/albums"),
                access_token,
                privacy,
            )
            .query(&[("limit", limit.to_string().as_str())]);
        self.send_json(request).await
    }

    /// GET related artists.
    pub async fn get_related_artists(
        &self,
        access_token: &str,
        privacy: &Privacy,
        artist_id: &str,
    ) -> SpotifyResult<Paging<Artist>> {
        let request = self.request(
            Method::GET,
            &format!("/artists/{artist_id}/related-artists"),
            access_token,
            privacy,
        );
        self.send_json(request).await
    }

    /// GET playlist details.
    pub async fn get_playlist(
        &self,
        access_token: &str,
        privacy: &Privacy,
        playlist_id: &str,
    ) -> SpotifyResult<Playlist> {
        let request = self.request(
            Method::GET,
            &format!("/playlists/{playlist_id}"),
            access_token,
            privacy,
        );
        self.send_json(request).await
    }

    /// GET the tracks of a playlist.
    pub async fn get_playlist_tracks(
        &self,
        access_token: &str,
        privacy: &Privacy,
        playlist_id: &str,
        limit: u32,
    ) -> SpotifyResult<Paging<Track>> {
        let request = self
            .request(
                Method::GET,
                &format!("/playlists/{playlist_id}/tracks"),
                access_token,
                privacy,
            )
            .query(&[("limit", limit.to_string().as_str())]);
        self.send_json(request).await
    }

    /// GET featured playlists.
    pub async fn get_featured_playlists(
        &self,
        access_token: &str,
        privacy: &Privacy,
        limit: u32,
    ) -> SpotifyResult<FeaturedPlaylists> {
        let request = self
            .request(
                Method::GET,
                "/browse/featured-playlists",
                access_token,
                privacy,
            )
            .query(&[("limit", limit.to_string().as_str())]);
        self.send_json(request).await
    }

    /// GET new album releases.
    pub async fn get_new_releases(
        &self,
        access_token: &str,
        privacy: &Privacy,
        limit: u32,
    ) -> SpotifyResult<NewReleases> {
        let request = self
            .request(Method::GET, "/browse/new-releases", access_token, privacy)
            .query(&[("limit", limit.to_string().as_str())]);
        self.send_json(request).await
    }

    /// GET browse categories.
    pub async fn get_categories(
        &self,
        access_token: &str,
        privacy: &Privacy,
        limit: u32,
    ) -> SpotifyResult<Vec<Category>> {
        let request = self
            .request(Method::GET, "/browse/categories", access_token, privacy)
            .query(&[("limit", limit.to_string().as_str())]);
        let response: Value = self.send_json(request).await?;
        Ok(response
            .get("categories")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default())
    }

    // -----------------------------------------------------------------------
    // Library
    // -----------------------------------------------------------------------

    /// GET the user's saved tracks.
    pub async fn get_saved_tracks(
        &self,
        access_token: &str,
        privacy: &Privacy,
        limit: u32,
    ) -> SpotifyResult<Paging<SavedTrack>> {
        let request = self
            .request(Method::GET, "/me/tracks", access_token, privacy)
            .query(&[("limit", limit.to_string().as_str())]);
        self.send_json(request).await
    }

    /// PUT save tracks to the user's library.
    pub async fn save_tracks(
        &self,
        access_token: &str,
        privacy: &Privacy,
        track_ids: &[String],
    ) -> SpotifyResult<()> {
        let request = self
            .request(Method::PUT, "/me/tracks", access_token, privacy)
            .json(&serde_json::json!({ "ids": track_ids }));
        self.send_empty(request).await
    }

    /// DELETE remove tracks from the user's library.
    pub async fn remove_saved_tracks(
        &self,
        access_token: &str,
        privacy: &Privacy,
        track_ids: &[String],
    ) -> SpotifyResult<()> {
        let request = self
            .request(Method::DELETE, "/me/tracks", access_token, privacy)
            .json(&serde_json::json!({ "ids": track_ids }));
        self.send_empty(request).await
    }

    /// GET the user's saved albums.
    pub async fn get_saved_albums(
        &self,
        access_token: &str,
        privacy: &Privacy,
        limit: u32,
    ) -> SpotifyResult<Vec<SavedAlbum>> {
        let request = self
            .request(Method::GET, "/me/albums", access_token, privacy)
            .query(&[("limit", limit.to_string().as_str())]);
        let response: Value = self.send_json(request).await?;
        Ok(response
            .get("items")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default())
    }

    /// GET the artists the user follows.
    pub async fn get_followed_artists(
        &self,
        access_token: &str,
        privacy: &Privacy,
        limit: u32,
    ) -> SpotifyResult<Paging<Artist>> {
        let request = self
            .request(Method::GET, "/me/following", access_token, privacy)
            .query(&[("type", "artist"), ("limit", limit.to_string().as_str())]);
        self.send_json(request).await
    }

    /// GET the user's playlists.
    pub async fn get_user_playlists(
        &self,
        access_token: &str,
        privacy: &Privacy,
        limit: u32,
    ) -> SpotifyResult<Vec<Playlist>> {
        let request = self
            .request(Method::GET, "/me/playlists", access_token, privacy)
            .query(&[("limit", limit.to_string().as_str())]);
        let response: Value = self.send_json(request).await?;
        Ok(response
            .get("items")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default())
    }

    /// GET the user's recently played tracks.
    pub async fn get_recently_played(
        &self,
        access_token: &str,
        privacy: &Privacy,
        limit: u32,
    ) -> SpotifyResult<Paging<Track>> {
        let request = self
            .request(
                Method::GET,
                "/me/player/recently-played",
                access_token,
                privacy,
            )
            .query(&[("limit", limit.to_string().as_str())]);
        self.send_json(request).await
    }
}

/// Turn a non-success response into an error, reading the body for context.
async fn check_status(response: Response) -> SpotifyResult<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status == StatusCode::UNAUTHORIZED {
        return Err(SpotifyError::Unauthorized);
    }
    if status == StatusCode::NOT_FOUND {
        return Err(SpotifyError::NotFound);
    }
    let body = response.text().await.unwrap_or_default();
    Err(SpotifyError::Status {
        code: status.as_u16(),
        body,
    })
}

/// Parse the `Retry-After` header, falling back to a default wait.
fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let raw = headers.get("retry-after")?.to_str().ok()?;
    let seconds: u64 = raw.trim().parse().ok()?;
    Some(Duration::from_secs(seconds.clamp(1, 30)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_is_parsed_and_clamped() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "5".parse().unwrap());
        assert_eq!(retry_after(&headers), Some(Duration::from_secs(5)));

        headers.insert("retry-after", "999".parse().unwrap());
        assert_eq!(retry_after(&headers), Some(Duration::from_secs(30)));

        headers.insert("retry-after", "soon".parse().unwrap());
        assert_eq!(retry_after(&headers), None);
    }

    #[test]
    fn client_shares_one_http_client() {
        let client = SpotifyClient::new().unwrap();
        let clone = client.clone();
        assert!(Arc::ptr_eq(&client.client, &clone.client));
    }

    #[test]
    fn request_headers_exclude_telemetry() {
        let client = SpotifyClient::new().unwrap();
        let privacy = Privacy::default();
        let request = client
            .request(Method::GET, "/me", "token", &privacy)
            .build()
            .unwrap();
        assert!(request.headers().get("x-spotify-client-info").is_none());
        assert_eq!(
            request.headers().get("authorization").unwrap(),
            "Bearer token"
        );
    }
}
