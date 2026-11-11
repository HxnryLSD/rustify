# Rustify — Project Plan

## Summary

Rustify is a lightweight unofficial Spotify client written in Rust, targeting
Linux, Windows, and macOS. It uses **egui** for the UI, **rodio** for audio
playback, and the Spotify Web API for catalogue and playback control.

## Context

- Project root: `/home/richter/Desktop/Rustify`.
- Chosen stack: Rust (edition 2024) + egui (GUI), rodio (audio), reqwest
  (HTTP), serde (JSON), tokio (async runtime), rustls (TLS).
- The application is a single crate with one binary target.

## System Impact

### Source of truth
- The Spotify Web API is the source of truth for track metadata, playlists,
  library, and playback state.
- The local config file (`config.toml`) stores user preferences.
- The OAuth token cache is stored locally (`tokens.json`).

### Data flow
1. The user signs in with OAuth (PKCE) or the web-player cookie exchange.
2. The Spotify API client fetches track/playlist/library data.
3. Playback is controlled on the user's Spotify Connect device.
4. The ad blocker vets tracks and refuses requests to ad hosts.
5. Bandwidth saver mode keeps the image loaders uninstalled and freezes
   animations.
6. The privacy layer keeps the header set minimal and writes no history.

### New state/transitions/invariants
- `AppState`: logged_out → authenticating → authenticated → playing/paused.
- `BandwidthSaverMode`: on/off — when on, image loaders are not installed and
  egui's animation time is zero.
- `AdBlocker`: active on all requests and tracks.
- Invariant: when bandwidth saver is on, no image HTTP request is issued.
- Invariant: no request carries a tracking header.
- Invariant: the token cache and config file are owner-readable only.

### Dependent components
- `spotify_client.rs` depends on auth tokens and the privacy policy.
- `ad_blocker.rs` is consulted by the UI before playback and by the request
  layer before a request leaves the process.
- `views.rs` depends on the bandwidth saver flag for rendering decisions.
- `config.rs` is the single source of truth read by all subsystems.

### Duplication avoided
- A single `Config` struct shared by auth, client, audio, and UI.
- A single `AdBlocker` instance used by both the player and the UI.
- A single `reqwest::Client` shared by every request, so connections are
  pooled.

### Smallest coherent solution
- One crate with a single binary; split into workspace crates only if the
  binary grows unwieldy.
- One `Config` TOML file; no database for v1.

## Approach

Documentation first, then implementation module by module, with the build
verified on every step.

## Changes

### Documentation files
- `README.md` — project overview, features, install, usage, privacy notes.
- `ARCHITECTURE.md` — system design, module boundaries, data flow, lifecycle.
- `FEATURES.md` — detailed spec for ad blocking, privacy, bandwidth saver,
  authentication, playback.
- `API.md` — Spotify Web API endpoints used, auth flow, rate limits.
- `ROADMAP.md` — release milestones.
- `CONFIG.md` — configuration file format and options.
- `PLAN.md` — this file.

### Implementation files
- `Cargo.toml` — dependencies: egui, eframe, egui_extras, image, rodio,
  reqwest, serde, serde_json, tokio, toml, dirs, url, hmac, sha1, sha2,
  data-encoding, rand.
- `src/main.rs` — eframe application entry point.
- `src/lib.rs` — public crate exports.
- `src/app.rs` — the `eframe::App` implementation.
- `src/app_state.rs` — shared state and navigation.
- `src/config.rs` — Config struct, TOML load/save, token cache, file modes.
- `src/auth.rs` — web-player sign-in, device id, session.
- `src/oauth.rs` — OAuth 2.0 authorization code + PKCE.
- `src/totp.rs` — the web player's one-time password and its live secret.
- `src/webplayer.rs` — the web-player API: client token, pathfinder.
- `src/spotify_client.rs` — the public Spotify Web API client.
- `src/models.rs` — API response types.
- `src/ad_blocker.rs` — ad host blocking and ad recognition.
- `src/audio_player.rs` — rodio audio playback.
- `src/bandwidth_saver.rs` — bandwidth saver policy.
- `src/privacy.rs` — privacy enforcement and telemetry stripping.
- `src/worker.rs` — background request/response plumbing.
- `src/ui.rs` — application object, request dispatch, API worker.
- `src/views.rs` — screen implementations.
- `src/login_view.rs` — sign-in screen.
- `src/youtube.rs` — YouTube search, matching, and stream resolution.
- `src/local_library.rs` — the local music folder.
- `src/error.rs` — error types.
- `tests/auth_flow.rs` — live sign-in tests against Spotify's servers.
- `tests/webplayer_flow.rs` — live web-player and YouTube tests.

## Verification

- `cargo build` — compiles without errors.
- `cargo build --release` — optimized build.
- `cargo test` — unit tests pass (config parsing, ad blocker matching, TOTP,
  the web-player response shapes, YouTube matching, URL handling, request
  headers).
- `cargo test -- --ignored` — live tests: the sign-in flow against Spotify's
  servers, the web-player API, and the YouTube search.
- `cargo clippy --all-targets -- -D warnings` — no warnings.
- `cargo fmt --check` — formatted.
- Manual: launch `cargo run`, complete the sign-in, browse, search, play a
  track, toggle bandwidth saver, and confirm images are disabled.
