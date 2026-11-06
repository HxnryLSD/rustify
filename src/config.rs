use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application configuration, persisted to TOML on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub auth: AuthConfig,
    pub privacy: PrivacyConfig,
    pub ad_blocker: AdBlockerConfig,
    pub bandwidth_saver: BandwidthSaverConfig,
    pub audio: AudioConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub private_session: bool,
    pub no_logging: bool,
    pub limit_telemetry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdBlockerConfig {
    pub enabled: bool,
    pub block_audio_ads: bool,
    pub block_banner_ads: bool,
    pub block_video_ads: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthSaverConfig {
    pub enabled: bool,
    pub disable_images: bool,
    pub disable_animations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub output_device: String,
    pub volume: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub window_width: f32,
    pub window_height: f32,
    pub theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auth: AuthConfig {
                client_id: String::new(),
                client_secret: String::new(),
                redirect_url: "http://localhost:8888/callback".to_string(),
            },
            privacy: PrivacyConfig {
                private_session: true,
                no_logging: true,
                limit_telemetry: true,
            },
            ad_blocker: AdBlockerConfig {
                enabled: true,
                block_audio_ads: true,
                block_banner_ads: true,
                block_video_ads: true,
            },
            bandwidth_saver: BandwidthSaverConfig {
                enabled: false,
                disable_images: true,
                disable_animations: true,
            },
            audio: AudioConfig {
                output_device: String::new(),
                volume: 0.8,
            },
            ui: UiConfig {
                window_width: 1024.0,
                window_height: 768.0,
                theme: "dark".to_string(),
            },
        }
    }
}

impl Config {
    /// Load config from disk, falling back to defaults if missing or invalid.
    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(data) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<Config>(&data) {
                    return cfg;
                }
            }
        }
        Config::default()
    }

    /// Save config to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Config::config_path()
            .ok_or_else(|| "Unable to determine config directory".to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = toml::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Platform-standard config file path.
    fn config_path() -> Option<PathBuf> {
        let mut path = dirs::config_dir()?;
        path.push("rustify");
        path.push("config.toml");
        Some(path)
    }
}

/// OAuth token cache persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCache {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64, // Unix timestamp seconds
}

impl TokenCache {
    pub fn load() -> Option<Self> {
        let path = Self::token_path()?;
        let data = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = TokenCache::token_path().ok_or("config dir unavailable")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        now >= self.expires_at
    }

    fn token_path() -> Option<PathBuf> {
        let mut path = dirs::config_dir()?;
        path.push("rustify");
        path.push("tokens.json");
        Some(path)
    }
}
