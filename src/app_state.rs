use crate::config::{Config, TokenCache};
use crate::models::{Album, Artist, Playlist, Track};
use std::sync::Arc;

/// Which screen the user is looking at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    Login,
    Home,
    Search,
    Library,
    Playlists,
    Album,
    Artist,
    Playlist,
    Local,
    Settings,
}

/// A detail view that was opened by clicking an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailTarget {
    Album(String),
    Artist(String),
    Playlist(String),
}

/// The item currently loaded in a detail view.
#[derive(Debug, Clone, Default)]
pub struct DetailState {
    pub album: Option<Album>,
    pub artist: Option<Artist>,
    pub playlist: Option<Playlist>,
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub related_artists: Vec<Artist>,
    pub loading: bool,
    pub error: Option<String>,
}

/// Everything the home screen shows.
#[derive(Debug, Clone, Default)]
pub struct HomeState {
    pub featured: Vec<Playlist>,
    pub new_releases: Vec<Album>,
    pub loading: bool,
    pub error: Option<String>,
    pub loaded: bool,
}

/// Everything the library screen shows.
#[derive(Debug, Clone, Default)]
pub struct LibraryState {
    pub saved_tracks: Vec<Track>,
    pub saved_albums: Vec<Album>,
    pub followed_artists: Vec<Artist>,
    /// Liked songs, loaded through the web-player API.
    pub liked_songs: Vec<Track>,
    pub loading: bool,
    pub liked_loading: bool,
    pub error: Option<String>,
    pub loaded: bool,
    pub liked_loaded: bool,
}

/// The user's playlists.
#[derive(Debug, Clone, Default)]
pub struct PlaylistsState {
    pub playlists: Vec<Playlist>,
    pub loading: bool,
    pub error: Option<String>,
    pub loaded: bool,
}

/// Results of the last search.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    pub playlists: Vec<Playlist>,
    pub loading: bool,
    pub error: Option<String>,
}

/// The device Spotify playback is running on.
#[derive(Debug, Clone, Default)]
pub struct DeviceState {
    pub devices: Vec<crate::models::Device>,
    pub active_id: Option<String>,
}

/// The state of the local audio player.
#[derive(Debug, Clone, Default)]
pub struct NowPlayingState {
    pub track_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub art_url: Option<String>,
    pub duration_ms: u32,
    pub position_ms: u64,
    pub is_playing: bool,
    pub is_buffering: bool,
    /// Whether the audio is played locally rather than on a Connect device.
    pub is_local: bool,
    /// Where the audio is coming from.
    pub source: AudioSource,
}

/// Where the audio Rustify is playing comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioSource {
    #[default]
    /// Nothing is playing.
    None,
    /// The Spotify preview clip.
    Preview,
    /// A YouTube stream.
    YouTube,
    /// A file on this computer.
    Local,
    /// A Spotify Connect device.
    Device,
}

impl AudioSource {
    /// A short label for the interface.
    pub fn label(self) -> &'static str {
        match self {
            AudioSource::None => "nothing",
            AudioSource::Preview => "preview",
            AudioSource::YouTube => "YouTube",
            AudioSource::Local => "local file",
            AudioSource::Device => "your device",
        }
    }
}

/// A transient message shown to the user.
#[derive(Debug, Clone)]
pub struct Notice {
    pub text: String,
    pub kind: NoticeKind,
}

/// The severity of a notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Warning,
    Error,
}

impl Notice {
    /// An informational message.
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: NoticeKind::Info,
        }
    }

    /// A warning message.
    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: NoticeKind::Warning,
        }
    }

    /// An error message.
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: NoticeKind::Error,
        }
    }
}

/// The complete application state.
pub struct AppState {
    pub config: Config,
    pub view: View,
    pub detail_target: Option<DetailTarget>,
    pub home: HomeState,
    pub library: LibraryState,
    pub playlists: PlaylistsState,
    pub search: SearchState,
    pub detail: DetailState,
    pub device: DeviceState,
    pub now_playing: NowPlayingState,
    pub local_tracks: Vec<crate::local_library::LocalTrack>,
    pub notice: Option<Notice>,
    pub logged_in: bool,
    pub logging_in: bool,
    pub logged_in_user: Option<String>,
}

impl AppState {
    /// Create the state from a loaded configuration.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            view: View::Login,
            detail_target: None,
            home: HomeState::default(),
            library: LibraryState::default(),
            playlists: PlaylistsState::default(),
            search: SearchState::default(),
            detail: DetailState::default(),
            device: DeviceState::default(),
            now_playing: NowPlayingState::default(),
            local_tracks: Vec::new(),
            notice: None,
            logged_in: false,
            logging_in: false,
            logged_in_user: None,
        }
    }

    /// Show a transient message.
    pub fn notify(&mut self, notice: Notice) {
        self.notice = Some(notice);
    }

    /// Clear the transient message.
    pub fn clear_notice(&mut self) {
        self.notice = None;
    }

    /// Navigate to a screen.
    pub fn navigate(&mut self, view: View) {
        let is_detail = matches!(view, View::Album | View::Artist | View::Playlist);
        self.view = view;
        if !is_detail {
            self.detail_target = None;
        }
    }

    /// Whether the given track is the one currently playing.
    pub fn is_current_track(&self, track_id: &str) -> bool {
        self.now_playing.track_id.as_deref() == Some(track_id)
    }
}

/// Shared, thread-safe access to the token cache.
pub type SharedTokens = Arc<std::sync::Mutex<Option<TokenCache>>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_clears_detail_target() {
        let mut state = AppState::new(Config::default());
        state.detail_target = Some(DetailTarget::Album("x".into()));
        state.navigate(View::Home);
        assert!(state.detail_target.is_none());

        state.detail_target = Some(DetailTarget::Album("x".into()));
        state.navigate(View::Album);
        assert!(state.detail_target.is_some());
    }

    #[test]
    fn notices_are_replaced_and_cleared() {
        let mut state = AppState::new(Config::default());
        state.notify(Notice::error("bad"));
        assert_eq!(state.notice.as_ref().unwrap().kind, NoticeKind::Error);
        state.notify(Notice::info("ok"));
        assert_eq!(state.notice.as_ref().unwrap().kind, NoticeKind::Info);
        state.clear_notice();
        assert!(state.notice.is_none());
    }
}
