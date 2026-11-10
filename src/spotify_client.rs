use crate::config::PrivacyConfig;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use std::time::Duration;

/// Spotify Web API client.
pub struct SpotifyClient {
    pub client: reqwest::Client,
    pub base_url: String,
}

impl Clone for SpotifyClient {
    fn clone(&self) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: self.base_url.clone(),
        }
    }
}

impl SpotifyClient {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            base_url: "https://api.spotify.com/v1".to_string(),
        })
    }

    /// Build default headers including auth and privacy headers.
    fn default_headers(&self, access_token: &str, _privacy: &PrivacyConfig) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {access_token}")).unwrap(),
        );
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        // Privacy: do not send telemetry/tracking headers.
        // private_session is enforced as a query param where applicable.
        headers
    }

    /// GET current user profile.
    pub async fn get_me(
        &self,
        access_token: &str,
        privacy: &PrivacyConfig,
    ) -> Result<CurrentUser, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/me", self.base_url);
        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers(access_token, privacy))
            .query(&[(
                "private_session",
                if privacy.private_session {
                    "true"
                } else {
                    "false"
                },
            )])
            .send()
            .await?
            .error_for_status()?;
        let user: CurrentUser = resp.json().await?;
        Ok(user)
    }

    /// GET current playback state.
    pub async fn get_playback_state(
        &self,
        access_token: &str,
        privacy: &PrivacyConfig,
    ) -> Result<Option<PlaybackState>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/me/player", self.base_url);
        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers(access_token, privacy))
            .query(&[(
                "private_session",
                if privacy.private_session {
                    "true"
                } else {
                    "false"
                },
            )])
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(None);
        }
        let resp = resp.error_for_status()?;
        let state: PlaybackState = resp.json().await?;
        Ok(Some(state))
    }

    /// PUT play / resume playback.
    pub async fn play(
        &self,
        access_token: &str,
        privacy: &PrivacyConfig,
        uris: Option<&[String]>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/me/player/play", self.base_url);
        let mut req = self
            .client
            .put(&url)
            .headers(self.default_headers(access_token, privacy))
            .query(&[(
                "private_session",
                if privacy.private_session {
                    "true"
                } else {
                    "false"
                },
            )]);
        if let Some(uris) = uris {
            let body = serde_json::json!({ "uris": uris });
            req = req.json(&body);
        }
        let resp = req.send().await?.error_for_status()?;
        let _ = resp.text().await?;
        Ok(())
    }

    /// PUT pause playback.
    pub async fn pause(
        &self,
        access_token: &str,
        privacy: &PrivacyConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/me/player/pause", self.base_url);
        let resp = self
            .client
            .put(&url)
            .headers(self.default_headers(access_token, privacy))
            .query(&[(
                "private_session",
                if privacy.private_session {
                    "true"
                } else {
                    "false"
                },
            )])
            .send()
            .await?
            .error_for_status()?;
        let _ = resp.text().await?;
        Ok(())
    }

    /// POST skip to next track.
    pub async fn next_track(
        &self,
        access_token: &str,
        privacy: &PrivacyConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/me/player/next", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.default_headers(access_token, privacy))
            .query(&[(
                "private_session",
                if privacy.private_session {
                    "true"
                } else {
                    "false"
                },
            )])
            .send()
            .await?
            .error_for_status()?;
        let _ = resp.text().await?;
        Ok(())
    }

    /// GET search tracks/albums/artists/playlists.
    pub async fn search(
        &self,
        access_token: &str,
        privacy: &PrivacyConfig,
        q: &str,
        r#type: &str,
        limit: u32,
    ) -> Result<SearchResponse, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/search", self.base_url);
        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers(access_token, privacy))
            .query(&[
                ("q", q),
                ("type", r#type),
                ("limit", limit.to_string().as_str()),
                (
                    "private_session",
                    if privacy.private_session {
                        "true"
                    } else {
                        "false"
                    },
                ),
            ])
            .send()
            .await?
            .error_for_status()?;
        let data: SearchResponse = resp.json().await?;
        Ok(data)
    }

    /// GET track details by ID.
    pub async fn get_track(
        &self,
        access_token: &str,
        privacy: &PrivacyConfig,
        track_id: &str,
    ) -> Result<Track, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/tracks/{track_id}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers(access_token, privacy))
            .send()
            .await?
            .error_for_status()?;
        let track: Track = resp.json().await?;
        Ok(track)
    }

    /// GET saved tracks (user library).
    pub async fn get_saved_tracks(
        &self,
        access_token: &str,
        privacy: &PrivacyConfig,
        limit: u32,
    ) -> Result<SavedTracksResponse, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/me/tracks", self.base_url);
        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers(access_token, privacy))
            .query(&[
                ("limit", limit.to_string().as_str()),
                (
                    "private_session",
                    if privacy.private_session {
                        "true"
                    } else {
                        "false"
                    },
                ),
            ])
            .send()
            .await?
            .error_for_status()?;
        let data: SavedTracksResponse = resp.json().await?;
        Ok(data)
    }
}

/// Current user profile.
#[derive(Debug, Clone, Deserialize)]
pub struct CurrentUser {
    pub id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub country: Option<String>,
    pub product: Option<String>,
}

/// Playback state returned by Spotify.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub progress_ms: Option<u32>,
    pub item: Option<PlaybackTrack>,
    pub device: Option<PlaybackDevice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackTrack {
    pub id: String,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Album,
    pub duration_ms: u32,
    pub preview_url: Option<String>,
    pub external_urls: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackDevice {
    pub id: String,
    pub name: String,
    pub active: bool,
}

/// Artist object.
#[derive(Debug, Clone, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

/// Album object.
#[derive(Debug, Clone, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub images: Vec<AlbumImage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlbumImage {
    pub url: String,
    pub height: Option<u32>,
    pub width: Option<u32>,
}

/// Track object.
#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Album,
    pub duration_ms: u32,
    pub preview_url: Option<String>,
}

/// Search response.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    pub tracks: Option<SearchTracks>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchTracks {
    pub items: Vec<Track>,
}

/// Saved tracks response.
#[derive(Debug, Clone, Deserialize)]
pub struct SavedTracksResponse {
    pub items: Vec<SavedTrack>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SavedTrack {
    pub added_at: String,
    pub track: Track,
}

impl Default for SpotifyClient {
    fn default() -> Self {
        Self::new().expect("failed to build SpotifyClient")
    }
}
