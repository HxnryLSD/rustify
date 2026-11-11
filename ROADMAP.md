# Rustify Roadmap

## Phase 1 — Documentation

- [x] PLAN.md — implementation roadmap
- [x] README.md — project overview
- [x] ARCHITECTURE.md — system design & module boundaries
- [x] FEATURES.md — feature specifications
- [x] API.md — Spotify API integration
- [x] CONFIG.md — configuration reference

## Phase 2 — MVP

- [x] Scaffold Rust project (`Cargo.toml`, `src/main.rs`, module files)
- [x] Config loading/saving (`config.rs`)
- [x] Sign-in flow (`auth.rs`, `oauth.rs`)
- [x] Spotify API client (`spotify_client.rs`, `webplayer.rs`)
- [x] egui UI shell: login, home, search, library views (`views.rs`)
- [x] Basic audio playback via rodio (`audio_player.rs`)
- [x] Build & run on Linux

## Phase 3 — Core Differentiators

- [x] Ad blocker — ad hosts refused, ads recognised by their markers (`ad_blocker.rs`)
- [x] Privacy enforcement — telemetry stripping, no local history (`privacy.rs`)
- [x] Extreme Bandwidth Saver Mode — image/animation disabling (`bandwidth_saver.rs`)
- [x] Token refresh on expiry
- [x] Playback controls: play/pause/skip/seek/volume/shuffle/repeat
- [x] Library: liked songs, saved tracks, albums, artists, playlists

## Phase 4 — Free accounts

- [x] The web-player API, so the library and the catalogue work on every plan
- [x] The signing secret read from the live bundle, so a rotation does not break the login
- [x] The client token, so the catalogue is not rate limited
- [x] YouTube as an audio source, with duration-based matching
- [x] `yt-dlp` and Invidious backends, with the preview clip as the fallback
- [x] Local music folder
- [x] The audio source shown in the player bar, with a preference in the settings

## Phase 5 — Polish & Cross-Platform

- [x] Windows & macOS builds — the manual ALSA link that broke them is gone
- [x] Settings UI (ad block, bandwidth saver, privacy, audio source, YouTube)
- [x] Error handling & user-facing error messages
- [x] Release binaries (GitHub Actions) with test, lint, and format gates
- [ ] Offline/cached data handling
- [ ] Keyboard shortcuts
- [ ] A player that remembers the position in a track

## Phase 6 — Future

- [ ] Spotify Connect device picker in the interface
- [ ] Crossfade & gapless playback
- [ ] Local playback of full Spotify tracks (needs a stream-decoding pipeline)
- [ ] Podcast support
- [ ] Lyrics display
- [ ] Plugin system for additional ad-block lists
- [ ] System tray integration

## Release Criteria

- **MVP:** Login works, search works, a track can be played end-to-end.
- **v1.0:** Ad blocking, privacy, and bandwidth saver are functional; stable on Linux.
- **v2.0:** Windows and macOS support, settings UI, polished UX.
