use crate::app_state::DetailState;
use crate::models::{Album, Artist, Device, PlaybackState, Playlist, Track};
use std::collections::HashMap;

/// A request the UI wants the background worker to perform.
#[derive(Debug, Clone)]
pub enum Command {
    LoadProfile,
    LoadHome,
    LoadLibrary,
    LoadPlaylists,
    LoadDevices,
    LoadPlaybackState,
    Search {
        query: String,
        types: String,
        limit: u32,
    },
    LoadAlbum {
        id: String,
    },
    LoadArtist {
        id: String,
    },
    LoadPlaylist {
        id: String,
    },
    PlayTrack {
        id: String,
    },
    /// Search through the web-player API.
    WebSearch {
        query: String,
        limit: u32,
    },
    /// Load the user's liked songs through the web-player API.
    WebLikedSongs {
        limit: u32,
    },
    /// Load the user's playlists through the web-player API.
    WebPlaylists {
        limit: u32,
    },
    /// Search YouTube for a track and resolve it to a stream.
    YouTubeMatch {
        track_id: String,
        title: String,
        artist: String,
        duration_ms: u32,
    },
    /// Download a track's preview and play it locally.
    PlayPreview {
        track_id: String,
        preview_url: String,
        title: String,
        artist: String,
        art_url: Option<String>,
        duration_ms: u32,
    },
    PlayContext {
        context_uri: String,
        offset: Option<u32>,
    },
    Pause,
    Next,
    Previous,
    Seek {
        position_ms: u32,
    },
    SetVolume {
        percent: u32,
    },
    TransferPlayback {
        device_id: String,
    },
    SaveTrack {
        id: String,
    },
    RemoveTrack {
        id: String,
    },
}

/// The payload a completed request carries back to the UI.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Payload {
    Profile {
        display_name: String,
        product: Option<String>,
    },
    Home {
        featured: Vec<Playlist>,
        new_releases: Vec<Album>,
    },
    Library {
        tracks: Vec<Track>,
        albums: Vec<Album>,
        artists: Vec<Artist>,
    },
    Playlists {
        playlists: Vec<Playlist>,
    },
    Devices {
        devices: Vec<Device>,
        active_id: Option<String>,
    },
    Playback {
        state: Option<PlaybackState>,
    },
    Search {
        tracks: Vec<Track>,
        albums: Vec<Album>,
        artists: Vec<Artist>,
        playlists: Vec<Playlist>,
    },
    Detail {
        detail: Box<DetailState>,
    },
    /// The remote device accepted a playback command.
    PlaybackCommand,
    /// The web-player search results.
    WebSearch {
        tracks: Vec<Track>,
        albums: Vec<Album>,
        artists: Vec<Artist>,
        playlists: Vec<Playlist>,
    },
    /// The user's liked songs.
    WebLikedSongs {
        tracks: Vec<Track>,
    },
    /// The user's playlists.
    WebPlaylists {
        playlists: Vec<Playlist>,
    },
    /// A YouTube match was found and resolved to a stream.
    YouTubeMatch {
        title: String,
        artist: String,
        video_url: String,
        stream_url: String,
        mime_type: String,
    },
    /// A preview was downloaded and is ready for local playback.
    Preview {
        track_id: String,
        title: String,
        artist: String,
        art_url: Option<String>,
        duration_ms: u32,
        /// The encoded audio.
        data: Vec<u8>,
    },
    /// A track was added to or removed from the library.
    LibraryChanged {
        track_id: String,
        saved: bool,
    },
}

/// A command paired with the identifier of its request.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: u64,
    pub command: Command,
}

/// A completed request.
#[derive(Debug, Clone)]
pub struct Response {
    /// Identifier of the request this belongs to.
    pub id: u64,
    /// The result.
    pub result: Result<Payload, ApiError>,
}

/// An error returned by a background request.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// The message shown to the user.
    pub message: String,
    /// The HTTP status, when the failure came from a response.
    pub status: Option<u16>,
}

impl ApiError {
    /// Build an error from a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: None,
        }
    }

    /// Whether the access token was rejected and the session must end.
    pub fn is_auth_failure(&self) -> bool {
        self.status == Some(401)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Tracks which UI slot each in-flight request belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    Profile,
    Home,
    Library,
    Playlists,
    Devices,
    Playback,
    Search,
    Detail,
    PlaybackCommand,
    LibraryChange,
    Preview,
    WebSearch,
    WebLikedSongs,
    WebPlaylists,
    YouTubeMatch,
}

/// Registry of in-flight requests.
#[derive(Debug, Default)]
pub struct RequestTracker {
    kinds: HashMap<u64, RequestKind>,
}

impl RequestTracker {
    /// Record a new request.
    pub fn start(&mut self, id: u64, kind: RequestKind) {
        self.kinds.insert(id, kind);
    }

    /// Look up and remove the kind of a completed request.
    pub fn finish(&mut self, id: u64) -> Option<RequestKind> {
        self.kinds.remove(&id)
    }

    /// Number of requests still in flight.
    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    /// Whether no request is in flight.
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_records_and_forgets_requests() {
        let mut tracker = RequestTracker::default();
        assert!(tracker.is_empty());

        tracker.start(1, RequestKind::Search);
        tracker.start(2, RequestKind::Home);
        assert_eq!(tracker.len(), 2);

        assert_eq!(tracker.finish(1), Some(RequestKind::Search));
        assert_eq!(tracker.finish(1), None);
        assert_eq!(tracker.len(), 1);
    }
}
