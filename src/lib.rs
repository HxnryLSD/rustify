//! Rustify — a lightweight, privacy-focused unofficial Spotify client.
//!
//! Modules:
//! - config — configuration and token persistence
//! - auth — web-player cookie exchange
//! - oauth — OAuth 2.0 authorization code flow with PKCE
//! - totp — time-based one-time password for the web-player exchange
//! - spotify_client — Spotify Web API client
//! - models — API response types
//! - ad_blocker — ad detection and blocking
//! - audio_player — rodio audio playback
//! - bandwidth_saver — Extreme Bandwidth Saver Mode
//! - privacy — privacy enforcement (private session, no logging, telemetry stripping)
//! - app_state — shared application state
//! - worker — background request/response plumbing
//! - ui — application shell
//! - views — screen implementations
//! - login_view — sign-in screen

pub mod ad_blocker;
pub mod app;
pub mod app_state;
pub mod audio_player;
pub mod auth;
pub mod bandwidth_saver;
pub mod config;
pub mod error;
pub mod local_library;
pub mod login_view;
pub mod models;
pub mod oauth;
pub mod privacy;
pub mod spotify_client;
pub mod totp;
pub mod ui;
pub mod views;
pub mod webplayer;
pub mod worker;
pub mod youtube;
