# Rustify — Project Plan

## Summary

Rustify is a lightweight unofficial Spotify client written in Rust, targeting Linux, Windows, and macOS. It uses **egui** for the UI, **rodio** for audio playback, and **sp_dc cookie authentication** for Spotify user authentication. Core differentiators: full ad blocking (audio/banner/video), privacy enforcement (private session, no logging, minimal telemetry), and an Extreme Bandwidth Saver Mode (disables images, animations, non-essential rendering).

This plan covers the documentation phase (markdown files) requested by the user, plus the scaffolding and implementation roadmap.

## Context

- Project root: `/home/richter/Desktop/Rustify` — empty directory.
- No existing code, Cargo.toml, or source files.
- Chosen stack: Rust + egui (GUI), rodio (audio), reqwest (HTTP), serde (JSON), tokio (async runtime).
- User decisions: egui UI, rodio audio, sp_dc cookie login, cross-platform (Linux/Windows/macOS).

## System Impact

### Source of truth
- Spotify Web API is the source of truth for track metadata, playlists, library, and playback state.
- Local config file (`config.toml`) stores user preferences (bandwidth saver mode, privacy flags, ad block settings).
- OAuth token cache stored locally (`tokens.json`) for session persistence.

### Data flow
1. User logs in via sp_dc cookie → access token stored locally.
2. Spotify API client (reqwest + serde) fetches track/playlist/library data.
3. Audio URLs resolved via Spotify API → decoded/played via rodio.
4. Ad blocker intercepts ad-related API responses and ad audio streams before playback.
5. Bandwidth saver mode strips image/animation rendering in egui.
6. Privacy layer forces `private_session=true`, strips telemetry headers, disables logging.

### New state/transitions/invariants
- `AppState`: logged_out → authenticating → authenticated → playing/paused/idle.
- `BandwidthSaverMode`: on/off — when on, image widgets render as placeholders, animations disabled.
- `AdBlocker`: active on all audio/banner/video requests; blocks known ad domains/patterns.
- Invariant: when bandwidth saver is on, no image HTTP requests are issued.
- Invariant: private session flag is always sent with Spotify API requests.

### Dependent components
- `spotify_client.rs` depends on auth tokens and privacy config.
- `audio_player.rs` depends on ad blocker (filters ad tracks) and bandwidth saver (may lower quality).
- `ui.rs` (egui) depends on bandwidth saver flag for rendering decisions.
- `config.rs` is the single source of truth read by all subsystems.

### Duplication avoided
- Single `Config` struct shared across auth, client, audio, and UI — no scattered feature flags.
- Single `AdBlocker` instance used by both audio player and UI for ad detection.

### Adjacent simplifications
- Bandwidth saver mode simplifies egui rendering (no image loading = fewer async tasks).
- Private session mode removes need to manage "recently played" history UI.

### Smallest coherent solution
- Start with a single binary (`rustify`) containing all modules; split into workspace crates only if the binary grows unwieldy.
- One `Config` TOML file; no database for v1 — JSON/TOML files for tokens and playlists cache.

## Approach

Documentation-first: write all markdown specs before code, so the implementation has a fixed contract. Then scaffold the Rust project and implement module by module.

## Changes

### Documentation files (this phase)
- `README.md` — project overview, features, install, usage, contributing.
- `ARCHITECTURE.md` — system design, module boundaries, data flow, lifecycle.
- `FEATURES.md` — detailed spec for ad blocking, privacy, bandwidth saver, OAuth, playback.
- `API.md` — Spotify Web API endpoints used, auth flow, rate limits.
- `ROADMAP.md` — release milestones (MVP, v1, v2).
- `CONFIG.md` — configuration file format and options.
- `PLAN.md` — this file (implementation roadmap).

### Implementation files (future phases)
- `Cargo.toml` — dependencies: egui, eframe, rodio, reqwest, serde, serde_json, tokio, toml, dirs.
- `src/main.rs` — egui application entry point.
- `src/lib.rs` — public crate exports.
- `src/config.rs` — Config struct, TOML load/save, defaults.
- `src/auth.rs` — sp_dc cookie exchange for access token, token storage.
- `src/spotify_client.rs` — Spotify Web API client (reqwest + serde).
- `src/ad_blocker.rs` — ad domain/pattern matching, stream interception.
- `src/audio_player.rs` — rodio playback, stream handling, ad-filtered playback.
- `src/bandwidth_saver.rs` — bandwidth saver mode logic, rendering policy.
- `src/privacy.rs` — private session enforcement, telemetry stripping, no-logging.
- `src/ui.rs` — egui UI: home, search, player, settings, library views.

## Verification

- `cargo build` — compiles without errors.
- `cargo test` — unit tests pass (config parsing, ad blocker matching, URL validation).
- Manual: launch `cargo run`, complete OAuth login, browse/search, play a track, toggle bandwidth saver, confirm images disabled.
- Edge cases: expired token refresh, offline mode, ad track skipped automatically, bandwidth saver persists across restart.