use serde::{Deserialize, Serialize};

/// Image attached to an album, artist, or playlist.
#[derive(Debug, Clone, Deserialize)]
pub struct Image {
    pub url: String,
    pub height: Option<u32>,
    pub width: Option<u32>,
}

/// Spotify image sizes, selected by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSize {
    Small,
    Medium,
    Large,
}

impl ImageSize {
    /// Spotify's URL template keyword for this size.
    pub fn keyword(self) -> &'static str {
        match self {
            ImageSize::Small => "small",
            ImageSize::Medium => "medium",
            ImageSize::Large => "large",
        }
    }
}

/// Apply a size to a Spotify CDN image URL.
///
/// Spotify encodes the requested size in the path, so the URL is rewritten
/// rather than re-fetched at a different width.
pub fn sized_image_url(url: &str, size: ImageSize) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            let mut segments: Vec<String> = parsed
                .path_segments()
                .map(|segs| segs.map(|s| s.to_string()).collect())
                .unwrap_or_default();
            if segments.len() > 1
                && let Some(last) = segments.last_mut()
            {
                *last = size.keyword().to_string();
            }
            let new_path = segments.join("/");
            parsed.set_path(&new_path);
            parsed.to_string()
        }
        Err(_) => url.to_string(),
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
    #[serde(default)]
    pub images: Vec<Image>,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

/// Links out to Spotify.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExternalUrls {
    #[serde(default)]
    pub spotify: Option<String>,
}

/// Artist object.
#[derive(Debug, Clone, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub images: Vec<Image>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub popularity: u32,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

/// Album object.
#[derive(Debug, Clone, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub images: Vec<Image>,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub album_type: Option<String>,
    #[serde(default)]
    pub total_tracks: u32,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

/// Track object.
#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub album: Option<Album>,
    #[serde(default)]
    pub duration_ms: u32,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub disc_number: Option<u32>,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub popularity: u32,
    #[serde(default)]
    pub is_local: bool,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

impl Track {
    /// Spotify URI for this track.
    pub fn uri(&self) -> String {
        format!("spotify:track:{}", self.id)
    }

    /// Primary artist name, or an empty string when the track has no artist.
    pub fn artist_name(&self) -> String {
        self.artists
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    /// Every artist name, joined for display.
    pub fn artist_names(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Album name, or an empty string when the track has no album.
    pub fn album_name(&self) -> String {
        self.album
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    /// Duration formatted as `m:ss` or `h:mm:ss`.
    pub fn duration_label(&self) -> String {
        format_duration(self.duration_ms)
    }
}

/// Playlist object.
#[derive(Debug, Clone, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub images: Vec<Image>,
    #[serde(default)]
    pub owner: Option<PlaylistOwner>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tracks: Paging<Track>,
    #[serde(default)]
    pub public: Option<bool>,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

/// Owner of a playlist.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistOwner {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// Cursor-paginated collection.
///
/// The `bound` attribute keeps serde from requiring `T: Default`, which the
/// per-field `default` attributes would otherwise imply.
#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "T: serde::de::DeserializeOwned"))]
pub struct Paging<T> {
    #[serde(default)]
    pub items: Vec<T>,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub total: u32,
}

/// Implemented by hand so the bound is not `T: Default`.
impl<T> Default for Paging<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            limit: 0,
            next: None,
            total: 0,
        }
    }
}

/// A track saved in the user's library.
#[derive(Debug, Clone, Deserialize)]
pub struct SavedTrack {
    pub added_at: String,
    pub track: Track,
}

/// Playback state returned by Spotify.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackState {
    #[serde(default)]
    pub is_playing: bool,
    #[serde(default)]
    pub progress_ms: Option<u32>,
    #[serde(default)]
    pub item: Option<PlaybackItem>,
    #[serde(default)]
    pub device: Option<PlaybackDevice>,
    #[serde(default)]
    pub shuffle_state: Option<ShuffleState>,
    #[serde(default)]
    pub repeat_state: Option<RepeatState>,
    #[serde(default)]
    pub context: Option<PlaybackContext>,
}

/// The track currently loaded in the player.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub album: Option<Album>,
    #[serde(default)]
    pub duration_ms: u32,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

/// The device Spotify is playing on.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackDevice {
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub volume_percent: Option<u32>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_private_session: bool,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub supports_volume: bool,
}

/// Shuffle state.
#[derive(Debug, Clone, Deserialize)]
pub struct ShuffleState {
    #[serde(default)]
    pub state: bool,
}

/// Repeat state.
#[derive(Debug, Clone, Deserialize)]
pub struct RepeatState {
    #[serde(default)]
    pub state: String,
}

/// The context (album, playlist, artist) playback is running in.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackContext {
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
}

/// A device the user can transfer playback to.
#[derive(Debug, Clone, Deserialize)]
pub struct Device {
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub volume_percent: Option<u32>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_private_session: bool,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub supports_volume: bool,
}

/// Search results across the requested types.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub tracks: Option<Paging<Track>>,
    #[serde(default)]
    pub albums: Option<Paging<Album>>,
    #[serde(default)]
    pub artists: Option<Paging<Artist>>,
    #[serde(default)]
    pub playlists: Option<Paging<Playlist>>,
}

/// A browse category.
#[derive(Debug, Clone, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icons: Vec<Image>,
}

/// Featured playlists shown on the home screen.
#[derive(Debug, Clone, Deserialize)]
pub struct FeaturedPlaylists {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub playlists: Paging<Playlist>,
}

/// New album releases.
#[derive(Debug, Clone, Deserialize)]
pub struct NewReleases {
    #[serde(default)]
    pub albums: Paging<Album>,
}

/// The user's playlists.
#[derive(Debug, Clone, Deserialize)]
pub struct UserPlaylists {
    #[serde(default)]
    pub items: Vec<Playlist>,
    #[serde(default)]
    pub total: u32,
}

/// The user's saved albums.
#[derive(Debug, Clone, Deserialize)]
pub struct SavedAlbums {
    #[serde(default)]
    pub items: Vec<SavedAlbum>,
    #[serde(default)]
    pub total: u32,
}

/// An album saved in the user's library.
#[derive(Debug, Clone, Deserialize)]
pub struct SavedAlbum {
    pub added_at: String,
    pub album: Album,
}

/// Artists the user follows.
#[derive(Debug, Clone, Deserialize)]
pub struct FollowedArtists {
    #[serde(default)]
    pub artists: Paging<Artist>,
}

/// Simple artist reference used by the album model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleArtist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

/// Format a duration in milliseconds as `m:ss` or `h:mm:ss`.
pub fn format_duration(millis: u32) -> String {
    let total_seconds = millis / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(61_000), "1:01");
        assert_eq!(format_duration(3_661_000), "1:01:01");
    }

    #[test]
    fn track_helpers_tolerate_missing_artists() {
        let track: Track =
            serde_json::from_str(r#"{"id":"abc","name":"Song","duration_ms":1000}"#).unwrap();
        assert_eq!(track.artist_name(), "");
        assert_eq!(track.artist_names(), "");
        assert_eq!(track.album_name(), "");
        assert_eq!(track.uri(), "spotify:track:abc");
    }

    #[test]
    fn rewrites_image_size() {
        let sized = sized_image_url("https://i.scdn.co/image/abcd1234", ImageSize::Small);
        assert_eq!(sized, "https://i.scdn.co/image/small");

        let unchanged = sized_image_url("not a url", ImageSize::Large);
        assert_eq!(unchanged, "not a url");
    }

    #[test]
    fn parses_track_with_minimal_fields() {
        let track: Track =
            serde_json::from_str(r#"{"id":"x","name":"n","artists":[],"album":null}"#).unwrap();
        assert!(track.artists.is_empty());
        assert!(!track.explicit);
    }
}
