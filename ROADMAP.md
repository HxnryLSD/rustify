# Rustify Roadmap

## Phase 1 — Documentation (Current)

- [x] PLAN.md — implementation roadmap
- [x] README.md — project overview
- [x] ARCHITECTURE.md — system design & module boundaries
- [x] FEATURES.md — feature specifications
- [x] API.md — Spotify API integration
- [x] CONFIG.md — configuration reference

## Phase 2 — MVP (Minimum Viable Product)

- [ ] Scaffold Rust project (`Cargo.toml`, `src/main.rs`, module files)
- [ ] Config loading/saving (`config.rs`)
- [ ] sp_dc cookie login flow (`auth.rs`)
- [ ] Basic Spotify API client (`spotify_client.rs`)
- [ ] egui UI shell: login, home, search views (`ui.rs`)
- [ ] Basic audio playback via rodio (`audio_player.rs`)
- [ ] Build & run on Linux

## Phase 3 — Core Differentiators

- [ ] Ad blocker — audio/banner/video ad detection & skipping (`ad_blocker.rs`)
- [ ] Privacy enforcement — private session, no logging, telemetry stripping (`privacy.rs`)
- [ ] Extreme Bandwidth Saver Mode — image/animation disabling (`bandwidth_saver.rs`)
- [ ] Token refresh on expiry
- [ ] Playback controls: play/pause/skip/seek
- [ ] Library: saved tracks, playlists

## Phase 4 — Polish & Cross-Platform

- [ ] Windows & macOS builds & testing
- [ ] Settings UI (toggle ad block, bandwidth saver, privacy)
- [ ] Offline/cached data handling
- [ ] Error handling & user-facing error messages
- [ ] Release binaries (GitHub Actions)

## Phase 5 — Future (Post-MVP)

- [ ] Spotify Connect device selection
- [ ] Crossfade & gapless playback
- [ ] Podcast support
- [ ] Lyrics display
- [ ] Plugin system for additional ad-block lists
- [ ] System tray integration

## Release Criteria

- **MVP:** Login works, search works, one track can be played end-to-end.
- **v1.0:** Ad blocking, privacy, and bandwidth saver all functional; stable on Linux.
- **v2.0:** Windows + macOS support, settings UI, polished UX.