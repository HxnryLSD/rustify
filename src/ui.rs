use crate::app_state::*;
use crate::audio_player::AudioPlayer;
use crate::bandwidth_saver::BandwidthSaver;
use crate::config::{Config, TokenCache};
use crate::privacy::Privacy;
use crate::spotify_client::SpotifyClient;
use crate::worker::{Command, Job, Payload, RequestKind, RequestTracker, Response};
use eframe::egui;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Shared access to the current access token.
///
/// The background worker reads it on every request; the UI updates it after
/// a login or a token refresh.
pub type TokenStore = Arc<Mutex<Option<String>>>;

/// The global token store.
///
/// A process-wide store keeps the worker and the UI in sync without having to
/// thread a handle through every call.
pub fn token_store() -> TokenStore {
    static STORE: std::sync::OnceLock<TokenStore> = std::sync::OnceLock::new();
    STORE.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

/// Replace the current access token.
pub fn set_access_token(token: Option<String>) {
    if let Ok(mut guard) = token_store().lock() {
        *guard = token;
    }
}

/// Read the current access token.
pub fn access_token() -> Option<String> {
    token_store().lock().ok().and_then(|guard| guard.clone())
}

/// Result of a login attempt, produced by a background task.
#[derive(Clone, Default)]
pub struct LoginOutcome {
    /// The error message, when the attempt failed.
    pub error: Option<String>,
    /// The OAuth URL the user has to open.
    pub oauth_url: Option<String>,
}

/// Download an audio stream.
async fn fetch_stream(url: &str, _mime_type: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|err| format!("the stream request failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("the stream server answered {}", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("the stream download failed: {err}"))?;
    Ok(bytes.to_vec())
}

/// A downloaded stream waiting to be played.
pub type PendingStream = (Vec<u8>, String, String);

/// The shared slot for a downloaded stream.
fn pending_stream() -> Arc<Mutex<Option<PendingStream>>> {
    static SLOT: std::sync::OnceLock<Arc<Mutex<Option<PendingStream>>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

/// Record a stream that finished downloading.
pub fn set_stream_ready(stream: Option<PendingStream>) {
    if let Ok(mut guard) = pending_stream().lock() {
        *guard = stream;
    }
}

/// Record that a stream download failed.
pub fn set_stream_error(error: String) {
    set_stream_ready(None);
    set_login_error(error);
}

/// The YouTube source, shared with the background worker.
fn youtube_slot() -> Option<crate::youtube::YouTubeSource> {
    static SLOT: std::sync::OnceLock<Arc<Mutex<Option<crate::youtube::YouTubeSource>>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| Arc::new(Mutex::new(None)))
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

/// Store the YouTube source for the background worker to use.
pub fn set_youtube_source(source: Option<crate::youtube::YouTubeSource>) {
    static SLOT: std::sync::OnceLock<Arc<Mutex<Option<crate::youtube::YouTubeSource>>>> =
        std::sync::OnceLock::new();
    let slot = SLOT.get_or_init(|| Arc::new(Mutex::new(None)));
    if let Ok(mut guard) = slot.lock() {
        *guard = source;
    }
}

/// The web-player client, shared with the background worker.
fn web_client_slot() -> Arc<Mutex<Option<crate::webplayer::WebPlayerClient>>> {
    static SLOT: std::sync::OnceLock<Arc<Mutex<Option<crate::webplayer::WebPlayerClient>>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

/// Store the web-player client for the background worker to use.
pub fn set_web_client(client: Option<crate::webplayer::WebPlayerClient>) {
    if let Ok(mut guard) = web_client_slot().lock() {
        *guard = client;
    }
}

/// Read the web-player client.
fn web_client() -> Option<crate::webplayer::WebPlayerClient> {
    web_client_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

/// The shared login outcome.
fn login_outcome() -> Arc<Mutex<LoginOutcome>> {
    static OUTCOME: std::sync::OnceLock<Arc<Mutex<LoginOutcome>>> = std::sync::OnceLock::new();
    OUTCOME
        .get_or_init(|| Arc::new(Mutex::new(LoginOutcome::default())))
        .clone()
}

/// Record a login failure.
pub fn set_login_error(error: String) {
    if let Ok(mut guard) = login_outcome().lock() {
        guard.error = Some(error);
    }
}

/// Record the OAuth authorization URL.
pub fn set_oauth_url(url: Option<String>) {
    if let Ok(mut guard) = login_outcome().lock() {
        guard.oauth_url = url;
    }
}

/// Take the pending login outcome, clearing it.
pub fn take_login_outcome() -> LoginOutcome {
    login_outcome()
        .lock()
        .map(|mut guard| std::mem::take(&mut *guard))
        .unwrap_or_default()
}

/// Application state shared between the egui UI and backend services.
pub struct RustifyApp {
    pub state: AppState,
    pub spotify: SpotifyClient,
    /// The web-player API client, which reaches the parts of Spotify that the
    /// public Web API reserves for Premium accounts.
    pub web: Option<crate::webplayer::WebPlayerClient>,
    /// The YouTube audio source.
    pub youtube: crate::youtube::YouTubeSource,
    pub audio: Option<AudioPlayer>,
    pub ad_blocker: crate::ad_blocker::AdBlocker,
    pub bandwidth_saver: BandwidthSaver,
    pub privacy: Privacy,

    /// Tokio runtime used for every background request.
    pub runtime: Option<tokio::runtime::Runtime>,
    /// Channel used to submit jobs to the background worker.
    pub jobs: Option<tokio::sync::mpsc::UnboundedSender<Job>>,
    /// Channel used to receive completed requests.
    pub responses: tokio::sync::mpsc::UnboundedReceiver<Response>,
    /// In-flight request bookkeeping.
    pub tracker: RequestTracker,
    /// Monotonic request id.
    next_request_id: AtomicU64,

    /// The `sp_dc` cookie typed by the user, kept out of the config file.
    pub sp_dc_input: String,
    /// The client id used for the OAuth PKCE flow.
    pub client_id_input: String,
    /// Whether the OAuth flow is waiting for the browser callback.
    pub oauth_pending: bool,
    /// Whether the cached session has already been restored.
    session_restored: bool,
    /// The OAuth authorization URL the user has to open.
    pub oauth_url: Option<String>,
}

impl RustifyApp {
    /// Build the application from a loaded configuration.
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        apply_theme(&cc.egui_ctx, &config.ui.theme);

        let ad_blocker = crate::ad_blocker::AdBlocker::from_config(
            config.ad_blocker.enabled,
            config.ad_blocker.block_audio_ads,
            config.ad_blocker.block_banner_ads,
            config.ad_blocker.block_video_ads,
        );
        let bandwidth_saver = crate::bandwidth_saver::BandwidthSaver::from_config(
            config.bandwidth_saver.enabled,
            config.bandwidth_saver.disable_images,
            config.bandwidth_saver.disable_animations,
        );
        let privacy = crate::privacy::Privacy::from_config(
            config.privacy.private_session,
            config.privacy.no_logging,
            config.privacy.limit_telemetry,
            config.privacy.no_local_history,
        );

        let audio = AudioPlayer::new().ok();
        let youtube = crate::youtube::YouTubeSource::new(crate::youtube::YouTubeSettings {
            enabled: config.youtube.enabled,
            invidious_instance: (!config.youtube.invidious_instance.is_empty())
                .then(|| config.youtube.invidious_instance.clone()),
            yt_dlp_path: (!config.youtube.yt_dlp_path.is_empty())
                .then(|| std::path::PathBuf::from(&config.youtube.yt_dlp_path)),
        });
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .ok();
        let (jobs, job_rx) = tokio::sync::mpsc::unbounded_channel();
        let (response_tx, responses) = tokio::sync::mpsc::unbounded_channel();

        set_youtube_source(Some(youtube.clone()));

        if let Some(runtime) = runtime.as_ref() {
            runtime.spawn(worker_loop(
                job_rx,
                response_tx,
                SpotifyClient::new().unwrap_or_default(),
                privacy,
            ));
        }

        Self {
            state: AppState::new(config),
            spotify: SpotifyClient::new().unwrap_or_default(),
            web: None,
            youtube,
            audio,
            ad_blocker,
            bandwidth_saver,
            privacy,
            runtime,
            jobs: Some(jobs),
            responses,
            tracker: RequestTracker::default(),
            next_request_id: AtomicU64::new(1),
            sp_dc_input: String::new(),
            client_id_input: String::new(),
            oauth_pending: false,
            oauth_url: None,
            session_restored: false,
        }
    }

    /// Register a new request and submit the job.
    pub fn dispatch(&mut self, kind: RequestKind, command: Command) {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        self.tracker.start(id, kind);
        if let Some(sender) = &self.jobs {
            let _ = sender.send(Job { id, command });
        }
    }

    /// Load the screen the user navigated to, if its data is not loaded yet.
    fn ensure_loaded(&mut self) {
        match self.state.view {
            View::Home => {
                if !self.state.home.loaded && !self.state.home.loading {
                    self.state.home.loading = true;
                    self.dispatch(RequestKind::Home, Command::LoadHome);
                }
            }
            View::Library => {
                if !self.state.library.loaded && !self.state.library.loading {
                    self.state.library.loading = true;
                    self.dispatch(RequestKind::Library, Command::LoadLibrary);
                }
                if !self.state.library.liked_loaded
                    && !self.state.library.liked_loading
                    && web_client().is_some()
                {
                    self.load_liked_songs();
                }
            }
            View::Playlists => {
                if !self.state.playlists.loaded && !self.state.playlists.loading {
                    self.state.playlists.loading = true;
                    self.dispatch(
                        RequestKind::WebPlaylists,
                        Command::WebPlaylists { limit: 100 },
                    );
                }
            }
            View::Album | View::Artist | View::Playlist
                if !self.state.detail.loading
                    && self.state.detail.error.is_none()
                    && self.state.detail.tracks.is_empty() =>
            {
                self.load_detail();
            }
            _ => {}
        }
    }

    /// Load the data for the currently open detail view.
    fn load_detail(&mut self) {
        let Some(target) = self.state.detail_target.clone() else {
            return;
        };
        self.state.detail = DetailState {
            loading: true,
            ..Default::default()
        };
        match target {
            DetailTarget::Album(id) => {
                self.dispatch(RequestKind::Detail, Command::LoadAlbum { id });
            }
            DetailTarget::Artist(id) => {
                self.dispatch(RequestKind::Detail, Command::LoadArtist { id });
            }
            DetailTarget::Playlist(id) => {
                self.dispatch(RequestKind::Detail, Command::LoadPlaylist { id });
            }
        }
    }

    /// Drain completed requests and apply them to the state.
    pub fn poll_responses(&mut self) {
        while let Ok(response) = self.responses.try_recv() {
            self.handle_response(response);
        }
        self.poll_login();
        self.poll_local_playback();
    }

    /// Play a stream that finished downloading.
    fn poll_pending_stream(&mut self) {
        let stream = pending_stream()
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some((data, title, artist)) = stream else {
            return;
        };
        let Some(audio) = self.audio.as_ref() else {
            self.state
                .notify(Notice::error("No audio output device is available"));
            self.state.now_playing.is_playing = false;
            return;
        };
        let duration = self.state.now_playing.duration_ms;
        let track_id = self.state.now_playing.track_id.clone().unwrap_or_default();
        if let Err(err) = audio.play_bytes(data, track_id, title, artist, None, duration) {
            self.state.now_playing.is_playing = false;
            self.state
                .notify(Notice::error(format!("Could not play the audio: {err}")));
        }
    }

    /// Keep the displayed state in step with the local audio player.
    fn poll_local_playback(&mut self) {
        if !self.state.now_playing.is_local {
            return;
        }
        if let Some(audio) = &self.audio {
            self.state.now_playing.is_playing = audio.is_playing();
        }
    }

    /// Restore the web-player session from the cached cookie.
    ///
    /// A returning user has their access token back from `tokens.json`, but
    /// the web-player session needs a fresh token and a client token, so it
    /// is established in the background.
    fn restore_web_session(&mut self) {
        if self.session_restored {
            return;
        }
        self.session_restored = true;

        let Some(tokens) = TokenCache::load() else {
            return;
        };
        if tokens.sp_dc_cookie.is_empty() {
            // An OAuth session uses the public API instead.
            return;
        }
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };

        let cookie = tokens.sp_dc_cookie.clone();
        runtime.spawn(async move {
            let auth = crate::auth::SpotifyAuth::with_live_secret().await;
            if let Ok(session) = auth.sign_in(&cookie).await {
                if let Err(err) = session.cache.save() {
                    eprintln!("could not save tokens: {err}");
                }
                crate::ui::set_access_token(Some(session.web_token.access_token.clone()));
                crate::ui::set_web_client(Some(session.client.clone()));
            }
        });
    }

    /// Check the result of a login attempt.
    fn poll_login(&mut self) {
        self.poll_pending_stream();
        self.restore_web_session();
        let outcome = crate::ui::take_login_outcome();
        if let Some(url) = outcome.oauth_url {
            self.oauth_url = Some(url);
            self.oauth_pending = true;
        }
        if let Some(error) = outcome.error {
            self.state.logging_in = false;
            self.state.notify(Notice::error(error));
        }

        // A token appeared, so the session is established.
        if !self.state.logged_in && crate::ui::access_token().is_some() {
            self.state.logging_in = false;
            self.state.logged_in = true;
            self.oauth_pending = false;
            self.navigate(View::Home);
            self.dispatch(RequestKind::Profile, Command::LoadProfile);
            self.dispatch(RequestKind::Devices, Command::LoadDevices);
            self.dispatch(RequestKind::Playback, Command::LoadPlaybackState);
        }

        // Keep the session alive for as long as the window is open.
        if self.state.logged_in {
            self.refresh_token_if_needed();
        }
    }

    /// Handle a completed background request.
    fn handle_response(&mut self, response: Response) {
        let Some(kind) = self.tracker.finish(response.id) else {
            return;
        };
        match response.result {
            Ok(payload) => self.handle_payload(kind, payload),
            Err(error) => {
                if error.is_auth_failure() {
                    self.sign_out();
                    self.state
                        .notify(Notice::error("Your session expired. Please sign in again."));
                    return;
                }
                self.handle_error(kind, error.message);
            }
        }
    }

    /// Apply a successful payload to the state.
    fn handle_payload(&mut self, kind: RequestKind, payload: Payload) {
        match (kind, payload) {
            (
                RequestKind::Profile,
                Payload::Profile {
                    display_name,
                    product,
                },
            ) => {
                self.state.logged_in = true;
                self.state.logged_in_user = Some(display_name);
                if let Some(product) = product {
                    self.state
                        .notify(Notice::info(format!("Signed in · {product}")));
                }
            }
            (
                RequestKind::Home,
                Payload::Home {
                    featured,
                    new_releases,
                },
            ) => {
                self.state.home.featured = featured;
                self.state.home.new_releases = new_releases;
                self.state.home.loading = false;
                self.state.home.loaded = true;
                self.state.home.error = None;
            }
            (
                RequestKind::Library,
                Payload::Library {
                    tracks,
                    albums,
                    artists,
                },
            ) => {
                self.state.library.saved_tracks = tracks;
                self.state.library.saved_albums = albums;
                self.state.library.followed_artists = artists;
                self.state.library.loading = false;
                self.state.library.loaded = true;
                self.state.library.error = None;
            }
            (RequestKind::Playlists, Payload::Playlists { playlists }) => {
                self.state.playlists.playlists = playlists;
                self.state.playlists.loading = false;
                self.state.playlists.loaded = true;
                self.state.playlists.error = None;
            }
            (RequestKind::Devices, Payload::Devices { devices, active_id }) => {
                self.state.device.devices = devices;
                self.state.device.active_id = active_id;
            }
            (RequestKind::Playback, Payload::Playback { state }) => {
                if let Some(state) = state {
                    self.state.now_playing.is_playing = state.is_playing;
                    self.state.now_playing.position_ms = state.progress_ms.unwrap_or(0) as u64;
                    if let Some(item) = state.item {
                        self.state.now_playing.track_id = Some(item.id.clone());
                        self.state.now_playing.title = item.name.clone();
                        self.state.now_playing.artist = item
                            .artists
                            .iter()
                            .map(|a| a.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        self.state.now_playing.duration_ms = item.duration_ms;
                        self.state.now_playing.art_url = item
                            .album
                            .as_ref()
                            .and_then(|album| album.images.first())
                            .map(|image| image.url.clone());
                    }
                } else {
                    self.state.now_playing.is_playing = false;
                }
            }
            (
                RequestKind::Search,
                Payload::Search {
                    tracks,
                    albums,
                    artists,
                    playlists,
                },
            ) => {
                self.state.search.tracks = tracks;
                self.state.search.albums = albums;
                self.state.search.artists = artists;
                self.state.search.playlists = playlists;
                self.state.search.loading = false;
                self.state.search.error = None;
            }
            (RequestKind::Detail, Payload::Detail { detail }) => {
                self.state.detail = *detail;
                self.state.detail.loading = false;
            }
            (
                RequestKind::WebSearch,
                Payload::WebSearch {
                    tracks,
                    albums,
                    artists,
                    playlists,
                },
            ) => {
                self.state.search.tracks = tracks;
                self.state.search.albums = albums;
                self.state.search.artists = artists;
                self.state.search.playlists = playlists;
                self.state.search.loading = false;
                self.state.search.error = None;
            }
            (RequestKind::WebLikedSongs, Payload::WebLikedSongs { tracks }) => {
                self.state.library.liked_songs = tracks;
                self.state.library.liked_loading = false;
                self.state.library.liked_loaded = true;
            }
            (RequestKind::WebPlaylists, Payload::WebPlaylists { playlists }) => {
                self.state.playlists.playlists = playlists;
                self.state.playlists.loading = false;
                self.state.playlists.loaded = true;
                self.state.playlists.error = None;
            }
            (
                RequestKind::YouTubeMatch,
                Payload::YouTubeMatch {
                    title,
                    artist,
                    video_url,
                    stream_url,
                    mime_type,
                },
            ) => {
                self.state.now_playing.source = AudioSource::YouTube;
                self.state.now_playing.is_playing = true;
                self.state.now_playing.is_local = true;
                self.state.now_playing.title = title.clone();
                self.state.now_playing.artist = artist.clone();

                let _ = video_url;
                self.play_stream(stream_url, mime_type, title, artist);
            }
            (RequestKind::PlaybackCommand, Payload::PlaybackCommand) => {
                self.state.notify(Notice::info("Playback command sent"));
            }
            (
                RequestKind::Preview,
                Payload::Preview {
                    track_id,
                    title,
                    artist,
                    art_url,
                    duration_ms,
                    data,
                },
            ) => {
                self.state.now_playing.track_id = Some(track_id.clone());
                self.state.now_playing.title = title.clone();
                self.state.now_playing.artist = artist.clone();
                self.state.now_playing.art_url = art_url.clone();
                self.state.now_playing.duration_ms = duration_ms;
                self.state.now_playing.is_playing = true;
                self.state.now_playing.is_local = true;

                match &self.audio {
                    Some(audio) => {
                        let result =
                            audio.play_bytes(data, track_id, title, artist, art_url, duration_ms);
                        if let Err(err) = result {
                            self.state.now_playing.is_playing = false;
                            self.state.notify(Notice::error(format!(
                                "Could not play the preview: {err}"
                            )));
                        }
                    }
                    None => {
                        self.state.now_playing.is_playing = false;
                        self.state
                            .notify(Notice::error("No audio output device is available"));
                    }
                }
            }
            (RequestKind::LibraryChange, Payload::LibraryChanged { track_id, saved }) => {
                self.state.notify(Notice::info(if saved {
                    format!("Saved {track_id} to your library")
                } else {
                    format!("Removed {track_id} from your library")
                }));
            }
            _ => {}
        }
    }

    /// Apply a failed request to the state.
    fn handle_error(&mut self, kind: RequestKind, message: String) {
        match kind {
            RequestKind::Home => {
                self.state.home.loading = false;
                self.state.home.error = Some(message.clone());
            }
            RequestKind::Library => {
                self.state.library.loading = false;
                self.state.library.error = Some(message.clone());
            }
            RequestKind::Playlists => {
                self.state.playlists.loading = false;
                self.state.playlists.error = Some(message.clone());
            }
            RequestKind::Search => {
                self.state.search.loading = false;
                self.state.search.error = Some(message.clone());
            }
            RequestKind::Detail => {
                self.state.detail.loading = false;
                self.state.detail.error = Some(message.clone());
            }
            RequestKind::Profile => {
                self.state.logged_in = false;
            }
            _ => {}
        }
        self.state.notify(Notice::error(message));
    }

    /// Play a track.
    ///
    /// Which source carries the sound depends on the preference and on what
    /// is available: a Spotify device first, then YouTube, then the preview
    /// clip, so a play button is never dead.
    pub fn play_track(&mut self, track: &crate::models::Track) {
        if !self
            .ad_blocker
            .is_ad_track(&track.name, &track.artist_name())
        {
            self.state
                .notify(Notice::warning("Track blocked by the ad blocker"));
            return;
        }

        let preference = self.state.config.library.audio_source;
        let has_device = self
            .state
            .device
            .devices
            .iter()
            .any(|device| device.is_active);

        match preference {
            crate::config::AudioSourcePreference::Device => self.play_on_device(track),
            crate::config::AudioSourcePreference::Preview => self.play_preview(track),
            crate::config::AudioSourcePreference::YouTube => self.play_on_youtube(track),
            crate::config::AudioSourcePreference::Local => {
                self.state
                    .notify(Notice::warning("Pick a file from the local music screen"));
            }
            crate::config::AudioSourcePreference::Automatic => {
                if has_device {
                    self.play_on_device(track);
                } else if self.state.config.youtube.enabled {
                    self.play_on_youtube(track);
                } else if track.preview_url.is_some() {
                    self.play_preview(track);
                } else {
                    self.state.notify(Notice::warning(
                        "No way to play this track: turn on YouTube in the settings, \
                         or play it on a Spotify device",
                    ));
                }
            }
        }
    }

    /// Play a track on the Spotify Connect device.
    pub fn play_on_device(&mut self, track: &crate::models::Track) {
        self.state.now_playing.track_id = Some(track.id.clone());
        self.state.now_playing.title = track.name.clone();
        self.state.now_playing.artist = track.artist_names();
        self.state.now_playing.art_url = track
            .album
            .as_ref()
            .and_then(|album| album.images.first())
            .map(|image| image.url.clone());
        self.state.now_playing.duration_ms = track.duration_ms;
        self.state.now_playing.is_playing = true;
        self.state.now_playing.is_local = false;
        self.state.now_playing.source = AudioSource::Device;

        self.dispatch(
            RequestKind::PlaybackCommand,
            Command::PlayTrack {
                id: track.id.clone(),
            },
        );
    }

    /// Play a track's preview locally through the audio output.
    ///
    /// The preview URL is vetted by the ad blocker before the download
    /// starts, and the audio is decoded and played by the local player.
    pub fn play_preview(&mut self, track: &crate::models::Track) {
        let Some(preview_url) = track.preview_url.clone() else {
            self.state
                .notify(Notice::warning("This track has no preview"));
            return;
        };

        if !self
            .ad_blocker
            .is_ad_track(&track.name, &track.artist_name())
        {
            self.state
                .notify(Notice::warning("Track blocked by the ad blocker"));
            return;
        }
        if self.ad_blocker.is_ad_url(&preview_url) {
            self.state
                .notify(Notice::warning("Preview blocked by the ad blocker"));
            return;
        }

        self.state.now_playing.is_buffering = true;
        self.state.now_playing.source = AudioSource::Preview;
        self.dispatch(
            RequestKind::Preview,
            Command::PlayPreview {
                track_id: track.id.clone(),
                preview_url,
                title: track.name.clone(),
                artist: track.artist_names(),
                art_url: track
                    .album
                    .as_ref()
                    .and_then(|album| album.images.first())
                    .map(|image| image.url.clone()),
                duration_ms: track.duration_ms,
            },
        );
    }

    /// Rescan the local music folder.
    pub fn rescan_local_library(&mut self) {
        let folder = self.state.config.library.local_folder.clone();
        if folder.is_empty() {
            return;
        }
        self.state.local_tracks = crate::local_library::scan_folder(std::path::Path::new(&folder));
    }

    /// Open a folder picker for the local music.
    pub fn choose_local_folder(&mut self) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            self.state.config.library.local_folder = folder.to_string_lossy().to_string();
            self.rescan_local_library();
        }
    }

    /// Play a file from the local library.
    pub fn play_local_track(&mut self, track: &crate::local_library::LocalTrack) {
        let Some(audio) = self.audio.as_ref() else {
            self.state
                .notify(Notice::error("No audio output device is available"));
            return;
        };
        let data = match std::fs::read(&track.path) {
            Ok(data) => data,
            Err(err) => {
                self.state
                    .notify(Notice::error(format!("Could not read the file: {err}")));
                return;
            }
        };
        let title = track.title.clone();
        let artist = track.artist.clone();
        if let Err(err) = audio.play_bytes(
            data,
            track.path.to_string_lossy().to_string(),
            title.clone(),
            artist.clone(),
            None,
            track.duration_ms,
        ) {
            self.state
                .notify(Notice::error(format!("Could not play the file: {err}")));
            return;
        }
        self.state.now_playing.source = AudioSource::Local;
        self.state.now_playing.title = title;
        self.state.now_playing.artist = artist;
        self.state.now_playing.is_playing = true;
        self.state.now_playing.is_local = true;
    }

    /// Look the track up on YouTube and play it.
    pub fn play_on_youtube(&mut self, track: &crate::models::Track) {
        if !self.state.config.youtube.enabled {
            self.state
                .notify(Notice::warning("YouTube is turned off in the settings"));
            return;
        }
        self.state.now_playing.is_buffering = true;
        self.state.now_playing.track_id = Some(track.id.clone());
        self.state.now_playing.title = track.name.clone();
        self.state.now_playing.artist = track.artist_names();
        self.state.now_playing.duration_ms = track.duration_ms;
        self.dispatch(
            RequestKind::YouTubeMatch,
            Command::YouTubeMatch {
                track_id: track.id.clone(),
                title: track.name.clone(),
                artist: track.artist_names(),
                duration_ms: track.duration_ms,
            },
        );
    }

    /// Search the catalogue.
    ///
    /// The web-player API reaches the whole catalogue, also for accounts
    /// without Premium. When there is no web-player session the public Web
    /// API is used instead.
    pub fn web_search(&mut self, query: &str) {
        if query.trim().is_empty() {
            return;
        }
        self.state.search.loading = true;
        let limit = self.state.config.ui.search_limit;

        if web_client().is_some() {
            self.dispatch(
                RequestKind::WebSearch,
                Command::WebSearch {
                    query: query.to_string(),
                    limit,
                },
            );
        } else {
            self.dispatch(
                RequestKind::Search,
                Command::Search {
                    query: query.to_string(),
                    types: "track,album,artist,playlist".to_string(),
                    limit,
                },
            );
        }
    }

    /// Load the user's liked songs.
    pub fn load_liked_songs(&mut self) {
        self.state.library.liked_loading = true;
        self.dispatch(
            RequestKind::WebLikedSongs,
            Command::WebLikedSongs { limit: 100 },
        );
    }

    /// Play an album or playlist through the Spotify Connect device.
    pub fn play_context(&mut self, context_uri: String, offset: Option<u32>) {
        self.dispatch(
            RequestKind::PlaybackCommand,
            Command::PlayContext {
                context_uri,
                offset,
            },
        );
    }

    /// Persist the current configuration and re-derive the runtime policies.
    pub fn save_config(&mut self) {
        self.ad_blocker = crate::ad_blocker::AdBlocker::from_config(
            self.state.config.ad_blocker.enabled,
            self.state.config.ad_blocker.block_audio_ads,
            self.state.config.ad_blocker.block_banner_ads,
            self.state.config.ad_blocker.block_video_ads,
        );
        self.bandwidth_saver = crate::bandwidth_saver::BandwidthSaver::from_config(
            self.state.config.bandwidth_saver.enabled,
            self.state.config.bandwidth_saver.disable_images,
            self.state.config.bandwidth_saver.disable_animations,
        );
        self.privacy = crate::privacy::Privacy::from_config(
            self.state.config.privacy.private_session,
            self.state.config.privacy.no_logging,
            self.state.config.privacy.limit_telemetry,
            self.state.config.privacy.no_local_history,
        );

        if let Some(audio) = &self.audio {
            audio.set_volume(self.state.config.audio.volume);
        }

        match self.state.config.save() {
            Ok(()) => self.state.notify(Notice::info("Settings saved")),
            Err(err) => self
                .state
                .notify(Notice::error(format!("Could not save settings: {err}"))),
        }
    }

    /// Load the screen the user navigated to.
    pub fn navigate(&mut self, view: View) {
        self.state.navigate(view);
        self.ensure_loaded();
    }

    /// Start the web-player cookie login.
    ///
    /// The exchange runs on the background runtime; the token is stored and
    /// the app switches to the home screen when it succeeds.
    pub fn begin_cookie_login(&mut self, sp_dc: String) {
        let Some(runtime) = self.runtime.as_ref() else {
            self.state
                .notify(Notice::error("Async runtime unavailable"));
            return;
        };

        self.state.logging_in = true;
        self.state.clear_notice();

        let sp_dc_for_task = sp_dc.clone();
        runtime.spawn(async move {
            // The signing secret is read from Spotify's live bundle so a
            // rotation does not break the sign-in.
            let auth = crate::auth::SpotifyAuth::with_live_secret().await;
            match auth.sign_in(&sp_dc_for_task).await {
                Ok(session) => {
                    if let Err(err) = session.cache.save() {
                        eprintln!("could not save tokens: {err}");
                    }
                    crate::ui::set_access_token(Some(session.web_token.access_token.clone()));
                    crate::ui::set_web_client(Some(session.client.clone()));
                }
                Err(err) => {
                    eprintln!("login failed: {err}");
                    crate::ui::set_login_error(err.to_string());
                }
            }
        });
    }

    /// Refresh the access token when it is about to expire.
    ///
    /// The OAuth flow uses the refresh token; the web-player flow re-mints a
    /// token with the stored `sp_dc` cookie.
    pub fn refresh_token_if_needed(&mut self) {
        let Some(tokens) = TokenCache::load() else {
            return;
        };
        if !tokens.expires_soon(300) {
            return;
        }
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };

        runtime.spawn(async move {
            let refreshed = if !tokens.refresh_token.is_empty() && !tokens.client_id.is_empty() {
                crate::oauth::refresh_token(&tokens.client_id, &tokens.refresh_token)
                    .await
                    .ok()
            } else if !tokens.sp_dc_cookie.is_empty() {
                let auth = crate::auth::SpotifyAuth::with_live_secret().await;
                auth.refresh(&tokens.sp_dc_cookie)
                    .await
                    .ok()
                    .map(|session| session.cache)
            } else {
                None
            };

            if let Some(refreshed) = refreshed {
                let _ = refreshed.save();
                crate::ui::set_access_token(Some(refreshed.access_token.clone()));
            }
        });
    }

    /// Download an audio stream and play it.
    ///
    /// The download runs on the background runtime so the interface stays
    /// responsive; the audio is handed to the player when it arrives.
    fn play_stream(&mut self, url: String, mime_type: String, title: String, artist: String) {
        let Some(runtime) = self.runtime.as_ref() else {
            self.state
                .notify(Notice::error("Async runtime unavailable"));
            return;
        };

        runtime.spawn(async move {
            match fetch_stream(&url, &mime_type).await {
                Ok(data) => crate::ui::set_stream_ready(Some((data, title, artist))),
                Err(err) => crate::ui::set_stream_error(err),
            }
        });
    }

    /// Start the OAuth PKCE login.
    pub fn begin_oauth_login(&mut self, client_id: String) {
        let Some(runtime) = self.runtime.as_ref() else {
            self.state
                .notify(Notice::error("Async runtime unavailable"));
            return;
        };

        self.state.logging_in = true;
        self.state.clear_notice();
        self.oauth_pending = true;

        runtime.spawn(async move {
            match crate::oauth::CallbackServer::bind(String::new()).await {
                Ok(server) => {
                    let redirect_uri = server.redirect_uri();
                    let session = crate::oauth::PkceSession::new(&redirect_uri);
                    let url = session.authorize_url(&client_id);

                    crate::login_view::open_in_browser(&url);
                    crate::ui::set_oauth_url(Some(url));

                    match server.wait_for_code().await {
                        Ok(code) => {
                            match crate::oauth::exchange_code(
                                &client_id,
                                &code,
                                &session.verifier,
                                &redirect_uri,
                            )
                            .await
                            {
                                Ok(tokens) => {
                                    if let Err(err) = tokens.save() {
                                        eprintln!("could not save tokens: {err}");
                                    }
                                    crate::ui::set_access_token(Some(tokens.access_token.clone()));
                                }
                                Err(err) => {
                                    eprintln!("oauth exchange failed: {err}");
                                    crate::ui::set_login_error(err.to_string());
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("oauth callback failed: {err}");
                            crate::ui::set_login_error(err.to_string());
                        }
                    }
                }
                Err(err) => {
                    eprintln!("could not start the callback server: {err}");
                    crate::ui::set_login_error(err.to_string());
                }
            }
        });
    }
}

/// Apply the configured theme to an egui context.
pub fn apply_theme(ctx: &egui::Context, theme: &str) {
    let visuals = if theme == "light" {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    ctx.set_visuals(visuals);
}

/// The background worker loop.
///
/// Owns the API client and executes jobs, sending results back over the
/// response channel.
async fn worker_loop(
    mut jobs: tokio::sync::mpsc::UnboundedReceiver<Job>,
    responses: tokio::sync::mpsc::UnboundedSender<Response>,
    spotify: SpotifyClient,
    privacy: Privacy,
) {
    let spotify = Arc::new(spotify);
    let privacy = Arc::new(privacy);

    tokio::spawn(async move {
        while let Some(job) = jobs.recv().await {
            let spotify = Arc::clone(&spotify);
            let privacy = Arc::clone(&privacy);
            let responses = responses.clone();

            tokio::spawn(async move {
                let result = execute(job.command, spotify, privacy).await;
                let _ = responses.send(Response { id: job.id, result });
            });
        }
    });
}

/// Convert an API error into a worker error, keeping the HTTP status.
fn api_error(err: crate::error::SpotifyError) -> crate::worker::ApiError {
    crate::worker::ApiError {
        message: err.to_string(),
        status: err.status(),
    }
}

/// Convert a transport error into a worker error.
fn transport_error(err: reqwest::Error) -> crate::worker::ApiError {
    crate::worker::ApiError {
        message: format!("network error: {err}"),
        status: err.status().map(|status| status.as_u16()),
    }
}

/// Execute a single command against the API.
async fn execute(
    command: Command,
    spotify: Arc<SpotifyClient>,
    privacy: Arc<Privacy>,
) -> Result<Payload, crate::worker::ApiError> {
    let token = access_token().ok_or_else(|| crate::worker::ApiError::new("Not signed in"))?;
    let privacy = privacy.as_ref();

    match command {
        Command::LoadProfile => {
            let user = spotify.get_me(&token, privacy).await.map_err(api_error)?;
            Ok(Payload::Profile {
                display_name: user.display_name.unwrap_or_else(|| user.id.clone()),
                product: user.product,
            })
        }
        Command::LoadHome => {
            let featured = spotify
                .get_featured_playlists(&token, privacy, 12)
                .await
                .map_err(api_error)?;
            let releases = spotify
                .get_new_releases(&token, privacy, 12)
                .await
                .map_err(api_error)?;
            Ok(Payload::Home {
                featured: featured.playlists.items,
                new_releases: releases.albums.items,
            })
        }
        Command::LoadLibrary => {
            let saved = spotify
                .get_saved_tracks(&token, privacy, 50)
                .await
                .map_err(api_error)?;
            let albums = spotify
                .get_saved_albums(&token, privacy, 50)
                .await
                .map_err(api_error)?;
            let followed = spotify
                .get_followed_artists(&token, privacy, 50)
                .await
                .map_err(api_error)?;
            Ok(Payload::Library {
                tracks: saved.items.into_iter().map(|item| item.track).collect(),
                albums: albums.into_iter().map(|item| item.album).collect(),
                artists: followed.items,
            })
        }
        Command::LoadPlaylists => {
            let playlists = spotify
                .get_user_playlists(&token, privacy, 50)
                .await
                .map_err(api_error)?;
            Ok(Payload::Playlists { playlists })
        }
        Command::LoadDevices => {
            let devices = spotify
                .get_devices(&token, privacy)
                .await
                .map_err(api_error)?;
            let active_id = devices
                .iter()
                .find(|device| device.is_active)
                .and_then(|device| device.id.clone());
            Ok(Payload::Devices { devices, active_id })
        }
        Command::LoadPlaybackState => {
            let state = spotify
                .get_playback_state(&token, privacy)
                .await
                .map_err(api_error)?;
            Ok(Payload::Playback { state })
        }
        Command::Search {
            query,
            types,
            limit,
        } => {
            let response = spotify
                .search(&token, privacy, &query, &types, limit)
                .await
                .map_err(api_error)?;
            Ok(Payload::Search {
                tracks: response.tracks.map(|p| p.items).unwrap_or_default(),
                albums: response.albums.map(|p| p.items).unwrap_or_default(),
                artists: response.artists.map(|p| p.items).unwrap_or_default(),
                playlists: response.playlists.map(|p| p.items).unwrap_or_default(),
            })
        }
        Command::LoadAlbum { id } => {
            let album = spotify
                .get_album(&token, privacy, &id)
                .await
                .map_err(api_error)?;
            let tracks = spotify
                .get_album_tracks(&token, privacy, &id, 50)
                .await
                .map_err(api_error)?;
            Ok(Payload::Detail {
                detail: Box::new(DetailState {
                    album: Some(album),
                    tracks: tracks.items,
                    loading: false,
                    ..Default::default()
                }),
            })
        }
        Command::LoadArtist { id } => {
            let artist = spotify
                .get_artist(&token, privacy, &id)
                .await
                .map_err(api_error)?;
            let top = spotify
                .get_artist_top_tracks(&token, privacy, &id, "US")
                .await
                .map_err(api_error)?;
            let related = spotify
                .get_related_artists(&token, privacy, &id)
                .await
                .map_err(api_error)?;
            Ok(Payload::Detail {
                detail: Box::new(DetailState {
                    artist: Some(artist),
                    tracks: top.items,
                    related_artists: related.items,
                    loading: false,
                    ..Default::default()
                }),
            })
        }
        Command::LoadPlaylist { id } => {
            let playlist = spotify
                .get_playlist(&token, privacy, &id)
                .await
                .map_err(api_error)?;
            let tracks = spotify
                .get_playlist_tracks(&token, privacy, &id, 50)
                .await
                .map_err(api_error)?;
            Ok(Payload::Detail {
                detail: Box::new(DetailState {
                    playlist: Some(playlist),
                    tracks: tracks.items,
                    loading: false,
                    ..Default::default()
                }),
            })
        }
        Command::PlayTrack { id } => {
            let uris = vec![format!("spotify:track:{id}")];
            spotify
                .play(&token, privacy, Some(&uris), None, None, None)
                .await
                .map_err(api_error)?;
            Ok(Payload::PlaybackCommand)
        }
        Command::WebSearch { query, limit } => {
            let client = web_client().ok_or_else(|| {
                crate::worker::ApiError::new("Not signed in to the Spotify web player")
            })?;
            let results = client.search(&query, limit).await.map_err(api_error)?;
            Ok(Payload::WebSearch {
                tracks: results.tracks,
                albums: results.albums,
                artists: results.artists,
                playlists: results.playlists,
            })
        }
        Command::WebLikedSongs { limit } => {
            let client = web_client().ok_or_else(|| {
                crate::worker::ApiError::new("Not signed in to the Spotify web player")
            })?;
            let tracks = client.get_liked_songs(limit).await.map_err(api_error)?;
            Ok(Payload::WebLikedSongs { tracks })
        }
        Command::WebPlaylists { limit } => {
            let client = web_client().ok_or_else(|| {
                crate::worker::ApiError::new("Not signed in to the Spotify web player")
            })?;
            let playlists = client.get_user_playlists(limit).await.map_err(api_error)?;
            Ok(Payload::WebPlaylists { playlists })
        }
        Command::YouTubeMatch {
            track_id,
            title,
            artist,
            duration_ms,
        } => {
            let source = youtube_slot().ok_or_else(|| {
                crate::worker::ApiError::new("the YouTube source is not available")
            })?;

            let track = crate::models::Track {
                id: track_id,
                name: title,
                artists: vec![crate::models::Artist {
                    id: String::new(),
                    name: artist,
                    images: vec![],
                    genres: vec![],
                    popularity: 0,
                    external_urls: Default::default(),
                }],
                album: None,
                duration_ms,
                preview_url: None,
                track_number: None,
                disc_number: None,
                explicit: false,
                popularity: 0,
                is_local: false,
                external_urls: Default::default(),
            };

            let candidates = source
                .search(&track)
                .await
                .map_err(crate::worker::ApiError::new)?;
            let video = source.find_match(&track, &candidates).ok_or_else(|| {
                crate::worker::ApiError::new("no YouTube video matched the track")
            })?;
            let stream = source
                .resolve_stream(&video)
                .await
                .map_err(crate::worker::ApiError::new)?;

            Ok(Payload::YouTubeMatch {
                title: track.name.clone(),
                artist: track.artist_names(),
                video_url: video.url,
                stream_url: stream.url,
                mime_type: stream.mime_type,
            })
        }
        Command::PlayPreview {
            track_id,
            preview_url,
            title,
            artist,
            art_url,
            duration_ms,
        } => {
            // The preview is fetched over the same client, so the ad blocker
            // can vet the URL before anything is downloaded.
            let client = spotify.http();
            let response = client
                .get(&preview_url)
                .send()
                .await
                .map_err(transport_error)?;
            if !response.status().is_success() {
                return Err(crate::worker::ApiError::new(format!(
                    "preview unavailable (HTTP {})",
                    response.status().as_u16()
                )));
            }
            let data = response.bytes().await.map_err(transport_error)?.to_vec();
            if data.is_empty() {
                return Err(crate::worker::ApiError::new("the preview was empty"));
            }
            Ok(Payload::Preview {
                track_id,
                title,
                artist,
                art_url,
                duration_ms,
                data,
            })
        }
        Command::PlayContext {
            context_uri,
            offset,
        } => {
            spotify
                .play(&token, privacy, None, Some(&context_uri), offset, None)
                .await
                .map_err(api_error)?;
            Ok(Payload::PlaybackCommand)
        }
        Command::Pause => {
            spotify.pause(&token, privacy).await.map_err(api_error)?;
            Ok(Payload::PlaybackCommand)
        }
        Command::Next => {
            spotify
                .next_track(&token, privacy)
                .await
                .map_err(api_error)?;
            Ok(Payload::PlaybackCommand)
        }
        Command::Previous => {
            spotify
                .previous_track(&token, privacy)
                .await
                .map_err(api_error)?;
            Ok(Payload::PlaybackCommand)
        }
        Command::Seek { position_ms } => {
            spotify
                .seek(&token, privacy, position_ms)
                .await
                .map_err(api_error)?;
            Ok(Payload::PlaybackCommand)
        }
        Command::SetVolume { percent } => {
            spotify
                .set_volume(&token, privacy, percent)
                .await
                .map_err(api_error)?;
            Ok(Payload::PlaybackCommand)
        }
        Command::TransferPlayback { device_id } => {
            spotify
                .transfer_playback(&token, privacy, &[device_id], true)
                .await
                .map_err(api_error)?;
            Ok(Payload::PlaybackCommand)
        }
        Command::SaveTrack { id } => {
            spotify
                .save_tracks(&token, privacy, std::slice::from_ref(&id))
                .await
                .map_err(api_error)?;
            Ok(Payload::LibraryChanged {
                track_id: id,
                saved: true,
            })
        }
        Command::RemoveTrack { id } => {
            spotify
                .remove_saved_tracks(&token, privacy, std::slice::from_ref(&id))
                .await
                .map_err(api_error)?;
            Ok(Payload::LibraryChanged {
                track_id: id,
                saved: false,
            })
        }
    }
}
