# Rustify Architecture

## Overview

Rustify is a single-binary desktop application. The UI layer (egui/eframe)
drives user interactions; the core layer (auth, Spotify client, web-player
client, audio player, ad blocker, privacy, bandwidth saver) handles business
logic and network I/O; config persists user preferences to disk.

## Module Boundaries

```
src/
  main.rs           — eframe/egui entry point, app loop
  lib.rs            — public crate exports
  app.rs            — the eframe::App implementation: panels, sidebar, player bar
  app_state.rs      — shared application state and navigation
  config.rs         — Config struct, TOML persistence, token cache, file permissions
  auth.rs           — web-player sign-in, device id, session
  oauth.rs          — OAuth 2.0 authorization code + PKCE, loopback callback server
  totp.rs           — the web player's one-time password and its live secret
  webplayer.rs      — the web-player API: client token, pathfinder operations
  spotify_client.rs — the public Spotify Web API client
  models.rs         — API response types
  ad_blocker.rs     — ad host blocking and ad recognition
  audio_player.rs   — rodio playback
  bandwidth_saver.rs — bandwidth saver policy
  privacy.rs        — telemetry stripping and header policy
  worker.rs         — background job and response plumbing
  ui.rs             — the application object, request dispatch, API worker
  views.rs          — screen implementations
  login_view.rs     — sign-in screen
  youtube.rs        — YouTube search, matching, and stream resolution
  local_library.rs  — the local music folder
  error.rs          — error types
```

## Two Spotify surfaces

Rustify speaks to Spotify twice, because Spotify itself has two APIs with
different reach:

- **The public Web API** (`spotify_client.rs`) — documented, stable, and
  Premium-only for playback and for your library.
- **The web-player API** (`webplayer.rs`) — undocumented, changeable, and
  reaches the library and the catalogue on every plan.

The interface prefers the web-player API and falls back to the public one, so
an OAuth session still works.

## Data Flow

1. **Auth:** The user picks a sign-in path on the login screen.
   - *Web player:* `auth.rs` reads the signing secret from Spotify's live
     bundle, signs the token request with the TOTP from `totp.rs`, and gets a
     web token plus a client token.
   - *OAuth:* `oauth.rs` starts a loopback listener, opens the browser, and
     exchanges the returned code for tokens.
   - Tokens are cached in `tokens.json`; the access token is published
   through the shared token store.
2. **API Request:** The UI dispatches a `Command` through the job channel in
   `worker.rs`. The background worker in `ui.rs` calls the web-player client
   or the public client, and returns a `Payload`.
3. **Ad Blocking:** `ad_blocker.rs` vets the track before playback and
   refuses requests to known ad hosts.
4. **Playback:** The audio source is chosen by `ui.rs` — a Spotify device,
   YouTube, a preview clip, or a local file — and the audio goes to
   `audio_player.rs`.
5. **Bandwidth Saver:** When enabled, `bandwidth_saver.rs` keeps the image
   loaders uninstalled and freezes egui's animations, so no image request is
   ever issued.
6. **Privacy:** `privacy.rs` keeps the header set to the two headers the API
   needs and strips anything that could carry tracking information.

## Concurrency Model

The egui thread never blocks. It dispatches a `Command` with an id over an
unbounded channel; the background worker executes it on a tokio runtime and
sends a `Response` with the same id back over a second channel. The UI drains
that channel every frame and routes each response to the state slot recorded
in the request tracker. Because every request carries its own id, several
requests can be in flight at once without overwriting each other.

Long-running work that is not a request — downloading a stream, for example —
finishes by publishing into a shared slot that the UI picks up on the next
frame.

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
AudioSource: Device / YouTube / Preview / Local / None.
PrivateSession: not a client-side state — it is enforced by Spotify's servers
(see `FEATURES.md`).

## Source of Truth

- Spotify: track/playlist/library metadata, playback state.
- `config.toml`: user preferences (bandwidth saver, ad block toggles, audio
  source, YouTube backends, local folder).
- `tokens.json`: access tokens + expiry.

## Lifecycle

1. App starts → loads `config.toml` → initializes `Config`.
2. Checks `tokens.json` → if a valid token exists, the session is restored.
3. User browses/searches → the API worker fetches data → `views.rs` renders.
4. User plays a track → `ad_blocker.rs` vets it → the chosen source plays it.
5. On shutdown → `eframe::App::on_exit` saves the config.

## Failure Modes

| Failure | Handling |
|---------|----------|
| Expired access token | The token is refreshed with the refresh token, or re-minted from the `sp_dc` cookie. |
| Network unavailable | The error is shown in the interface; the current screen keeps its data. |
| Ad track detected | Playback is refused with a notice. |
| Audio device unavailable | The application still starts; local playback is disabled and the remote device controls remain available. |
| Invalid config file | The broken file is kept as `config.toml.broken`, the defaults are used, and the problem is reported on stderr. |
| Rate limited (429) | The request is retried up to three times, honouring `Retry-After`. |
| Rejected token (401) | The session ends and the sign-in screen comes back. |
| YouTube finds no match | The request falls back to the preview clip. |
| YouTube has no backend | The setting says so, and the preview clip is used instead. |
| The signing secret rotates | It is read from the live bundle at sign-in; the bundled copy is the fallback. |
