# Rustify Architecture

## Overview

Rustify is a single-binary desktop application. The UI layer (egui/eframe) drives user interactions; the core layer (auth, Spotify client, audio player, ad blocker, privacy, bandwidth saver) handles business logic and network I/O; config persists user preferences to disk.

## Module Boundaries

```
src/
  main.rs          — eframe/egui entry point, app loop
  lib.rs           — public crate exports
  config.rs        — Config struct, TOML persistence, defaults
  auth.rs          — OAuth 2.0 flow, token storage & refresh
  spotify_client.rs — Spotify Web API client (reqwest + serde)
  ad_blocker.rs    — ad domain/pattern matching, stream interception
  audio_player.rs  — rodio playback, stream handling
  bandwidth_saver.rs — bandwidth saver mode logic & rendering policy
  privacy.rs       — private session enforcement, telemetry stripping
  ui.rs            — egui views: home, search, player, settings, library
```

## Data Flow

1. **Auth:** User clicks "Log in with Spotify" → egui opens system browser → OAuth redirect captures authorization code → `auth.rs` exchanges code for access/refresh tokens → tokens saved to `tokens.json`.
2. **API Request:** `spotify_client.rs` attaches `Authorization: Bearer <token>` and privacy headers (`private_session=true`) to every Spotify Web API call.
3. **Ad Blocking:** Before any audio stream starts, `ad_blocker.rs` checks the track/stream URL against known ad domains and patterns. Ad tracks are skipped automatically.
4. **Playback:** `audio_player.rs` receives a clean (non-ad) stream URL and plays it via rodio.
5. **Bandwidth Saver:** When enabled, `bandwidth_saver.rs` signals `ui.rs` to render image placeholders instead of loading images and to disable all animations.
6. **Privacy:** `privacy.rs` ensures no logging, no telemetry headers, and `private_session` on all Spotify API calls.

## State Machine

```
AppState:
  LoggedOut
    --(login success)--> Authenticated
  Authenticated
    --(play track)--> Playing
    --(pause)--> Paused
    Playing/Paused
    --(stop/logout)--> LoggedOut
```

BandwidthSaverMode: On / Off (persisted in config).
AdBlocker: Enabled / Disabled (persisted in config).
PrivateSession: Always On (enforced, not user-configurable).

## Source of Truth

- Spotify Web API: track/playlist/library metadata, playback state.
- `config.toml`: user preferences (bandwidth saver, ad block toggles).
- `tokens.json`: OAuth access/refresh tokens + expiry.

## Lifecycle

1. App starts → loads `config.toml` → initializes `Config`.
2. Checks `tokens.json` → if valid token exists, skip login; else show login screen.
3. User browses/searches → `spotify_client.rs` fetches data → `ui.rs` renders.
4. User plays a track → `ad_blocker.rs` vets the stream → `audio_player.rs` plays.
5. On shutdown → saves config + tokens → exits.

## Failure Modes

| Failure | Handling |
|---------|----------|
| Expired access token | Auto-refresh using refresh token; if refresh fails, return to login screen. |
| Network unavailable | Show offline state; cached data (if any) shown; playback disabled. |
| Ad track detected | Skip to next track; log nothing (privacy). |
| Audio device unavailable | Show error in UI; allow user to select different output device. |
| Invalid config file | Fall back to defaults; prompt user to fix config. |