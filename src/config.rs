use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Application configuration, persisted to TOML on disk.
///
/// Every field defaults to a sensible value, so a missing or partial file never
/// discards the user's settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub auth: AuthConfig,
    pub privacy: PrivacyConfig,
    pub ad_blocker: AdBlockerConfig,
    pub bandwidth_saver: BandwidthSaverConfig,
    pub audio: AudioConfig,
    pub ui: UiConfig,
    pub youtube: YouTubeConfig,
    pub library: LibraryConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    /// `sp_dc` cookie value used for the web-player token exchange.
    ///
    /// Never written to `config.toml`; it is kept in a separate `0600` file.
    #[serde(skip)]
    pub sp_dc_cookie: String,
    /// Spotify application client id, used by the OAuth PKCE flow.
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacyConfig {
    /// Force private session on all Spotify API calls.
    ///
    /// The web player honours this flag; the public Web API ignores unknown
    /// query parameters, so it is only meaningful for web-player endpoints.
    pub private_session: bool,
    /// Disable all local logging.
    pub no_logging: bool,
    /// Limit telemetry and analytics headers.
    pub limit_telemetry: bool,
    /// Do not persist listening history locally.
    pub no_local_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AdBlockerConfig {
    /// Enable ad blocking globally.
    pub enabled: bool,
    /// Block audio ads (ad tracks skipped before playback).
    pub block_audio_ads: bool,
    /// Block banner ads in the UI.
    pub block_banner_ads: bool,
    /// Block video ads.
    pub block_video_ads: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BandwidthSaverConfig {
    /// Enable Extreme Bandwidth Saver Mode.
    pub enabled: bool,
    /// Disable most image rendering (album art, artist images, covers).
    pub disable_images: bool,
    /// Disable all animations.
    pub disable_animations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Preferred output device (empty = system default).
    pub output_device: String,
    /// Volume (0.0 to 1.0).
    pub volume: f32,
    /// Prebuffer size in seconds for audio streams.
    pub prebuffer_seconds: f32,
}

/// The YouTube audio source settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeConfig {
    /// Whether YouTube may be used as an audio source.
    pub enabled: bool,
    /// An Invidious instance to use, when the user configured one.
    pub invidious_instance: String,
    /// The `yt-dlp` executable to run, when it was found.
    pub yt_dlp_path: String,
}

/// Where the audio comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioSourcePreference {
    /// Let Rustify decide, preferring the best available source.
    #[default]
    Automatic,
    /// Play on a Spotify Connect device.
    Device,
    /// Use the Spotify preview clip.
    Preview,
    /// Use YouTube.
    YouTube,
    /// Use a local file.
    Local,
}

/// The local library settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryConfig {
    /// Which source the audio comes from.
    pub audio_source: AudioSourcePreference,
    /// The folder scanned for local music.
    pub local_folder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub window_width: f32,
    pub window_height: f32,
    /// Theme: "light" or "dark".
    pub theme: String,
    /// Show the album art panel on the right.
    pub show_cover_panel: bool,
    /// Number of search results to request per query.
    pub search_limit: u32,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            private_session: true,
            no_logging: true,
            limit_telemetry: true,
            no_local_history: true,
        }
    }
}

impl Default for AdBlockerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            block_audio_ads: true,
            block_banner_ads: true,
            block_video_ads: true,
        }
    }
}

impl Default for BandwidthSaverConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            disable_images: true,
            disable_animations: true,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_device: String::new(),
            volume: 0.8,
            prebuffer_seconds: 8.0,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            window_width: 1180.0,
            window_height: 760.0,
            theme: "dark".to_string(),
            show_cover_panel: true,
            search_limit: 30,
        }
    }
}

impl Config {
    /// Load config from disk, falling back to defaults if missing or invalid.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Config::default();
        };
        Self::load_from(&path)
    }

    /// Load config from an explicit path.
    pub fn load_from(path: &Path) -> Self {
        let Ok(data) = fs::read_to_string(path) else {
            return Config::default();
        };
        // A malformed file must not wipe the user's settings, so keep the
        // broken copy around and start from defaults instead.
        match toml::from_str::<Config>(&data) {
            Ok(cfg) => cfg,
            Err(err) => {
                let backup = path.with_extension("toml.broken");
                let _ = fs::write(&backup, &data);
                eprintln!(
                    "config: {} is invalid ({err}); backed up to {}",
                    path.display(),
                    backup.display()
                );
                Config::default()
            }
        }
    }

    /// Save config to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path().ok_or("Unable to determine config directory")?;
        self.save_to(&path)
    }

    /// Save config to an explicit path.
    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = toml::to_string_pretty(self)?;
        write_private(path, data.as_bytes())
    }

    /// Platform-standard config file path.
    ///
    /// `RUSTIFY_CONFIG_PATH` overrides the location.
    pub fn config_path() -> Option<PathBuf> {
        if let Ok(custom) = std::env::var("RUSTIFY_CONFIG_PATH")
            && !custom.is_empty()
        {
            return Some(PathBuf::from(custom));
        }
        let mut path = dirs::config_dir()?;
        path.push("rustify");
        path.push("config.toml");
        Some(path)
    }
}

/// Write a file that only the current user can read.
pub fn write_private(path: &Path, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, data)?;
    restrict_permissions(path)?;
    Ok(())
}

/// Restrict a file to owner read/write, best-effort.
#[cfg(unix)]
pub fn restrict_permissions(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Restrict a file to owner read/write, best-effort.
#[cfg(not(unix))]
pub fn restrict_permissions(_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

/// OAuth token cache persisted to disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TokenCache {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix timestamp in seconds.
    pub expires_at: i64,
    /// Spotify application client id, needed to refresh a PKCE token.
    pub client_id: String,
    /// `sp_dc` cookie, kept so the web-player flow can re-mint a token.
    pub sp_dc_cookie: String,
}

impl TokenCache {
    /// Load the token cache, if present and readable.
    pub fn load() -> Option<Self> {
        let path = Self::token_path()?;
        Self::load_from(&path)
    }

    /// Load the token cache from an explicit path.
    pub fn load_from(path: &Path) -> Option<Self> {
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Persist the token cache with owner-only permissions.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::token_path().ok_or("config dir unavailable")?;
        self.save_to(&path)
    }

    /// Persist the token cache to an explicit path with owner-only permissions.
    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        write_private(path, data.as_bytes())
    }

    /// Whether the access token has expired.
    pub fn is_expired(&self) -> bool {
        now_unix() >= self.expires_at
    }

    /// Whether the access token expires within the next `margin` seconds.
    pub fn expires_soon(&self, margin: i64) -> bool {
        now_unix() + margin >= self.expires_at
    }

    /// Path of the token cache file.
    pub fn token_path() -> Option<PathBuf> {
        if let Ok(custom) = std::env::var("RUSTIFY_CONFIG_PATH")
            && !custom.is_empty()
        {
            let mut path = PathBuf::from(custom);
            path.set_extension("tokens.json");
            return Some(path);
        }
        let mut path = dirs::config_dir()?;
        path.push("rustify");
        path.push("tokens.json");
        Some(path)
    }
}

/// Current unix timestamp in seconds.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_config_keeps_defaults_for_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[privacy]\nno_logging = false\n").unwrap();

        let cfg = Config::load_from(&path);
        assert!(!cfg.privacy.no_logging);
        // Untouched sections must fall back to their defaults, not to zero values.
        assert!(cfg.privacy.private_session);
        assert!(cfg.ad_blocker.enabled);
        assert_eq!(cfg.ui.theme, "dark");
        assert!((cfg.audio.volume - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn broken_config_falls_back_to_defaults_and_keeps_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "this is not toml = = =").unwrap();

        let cfg = Config::load_from(&path);
        assert_eq!(cfg.ui.theme, "dark");
        assert!(path.with_extension("toml.broken").exists());
    }

    #[test]
    fn round_trips_through_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.audio.volume = 0.42;
        cfg.bandwidth_saver.enabled = true;
        cfg.save_to(&path).unwrap();

        let loaded = Config::load_from(&path);
        assert!((loaded.audio.volume - 0.42).abs() < f32::EPSILON);
        assert!(loaded.bandwidth_saver.enabled);
    }

    #[test]
    fn token_cache_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tokens.json");

        let cache = TokenCache {
            access_token: "secret".into(),
            expires_at: now_unix() + 3600,
            ..Default::default()
        };
        cache.save_to(&path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(
                mode & 0o777,
                0o600,
                "token cache must not be world-readable"
            );
        }

        let loaded = TokenCache::load_from(&path).unwrap();
        assert_eq!(loaded.access_token, "secret");
        assert!(!loaded.is_expired());
    }

    #[test]
    fn expired_token_is_detected() {
        let cache = TokenCache {
            expires_at: now_unix() - 1,
            ..Default::default()
        };
        assert!(cache.is_expired());
        assert!(cache.expires_soon(60));
    }
}
