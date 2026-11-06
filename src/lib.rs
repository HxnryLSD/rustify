//! Rustify — a lightweight unofficial Spotify client written in Rust.
//!
//! Modules:
//! - config — configuration loading/saving
//! - auth — OAuth 2.0 authentication
//! - spotify_client — Spotify Web API client
//! - ad_blocker — ad detection & blocking
//! - audio_player — rodio audio playback
//! - bandwidth_saver — Extreme Bandwidth Saver Mode
//! - privacy — privacy enforcement (private session, no logging, telemetry stripping)
//! - ui — egui user interface
//! - oauth_server — local HTTP listener for Spotify OAuth callback

pub mod ad_blocker;
pub mod audio_player;
pub mod auth;
pub mod bandwidth_saver;
pub mod config;
pub mod oauth_server;
pub mod privacy;
pub mod spotify_client;
pub mod ui;
