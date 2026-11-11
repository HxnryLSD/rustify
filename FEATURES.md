# Rustify Features

## 1. Spotify catalogue and library

- Browse featured playlists, new releases, and categories.
- Search tracks, albums, artists, and playlists.
- View track details, album art, artist info, and album/playlist track lists.
- **Your liked songs and your playlists, on every plan.** Spotify reserves
  those for Premium in the public API, so Rustify reads them through the same
  API the web player uses.
- Save and remove tracks from your library.
- Play / pause / skip / seek / volume / shuffle / repeat on a Spotify device.
- Transfer playback to another device on the account.

## 2. Four ways to play

| Source | What it is | Needs |
|---|---|---|
| **Your device** | Playback on a Spotify Connect device | Premium |
| **YouTube** | Rustify plays the track locally from YouTube | Nothing |
| **Preview clip** | Spotify's own ~30 second snippet | Nothing |
| **Local files** | Your own music | Nothing |

Rustify picks the best available source, and falls back so that a play
button is never dead. The source in use is shown in the player bar.

### YouTube matching

A Spotify track is matched to a YouTube video by:

- **Duration** — the closest match wins, and anything more than 8% off is
  ruled out. This removes one-hour loops, sped-up edits, and explainer
  videos.
- **Title and artist** in the video's title.
- **The artist's own channel**, which is a good sign.
- Versions that are not the studio recording — sped up, remix, live, cover,
  karaoke, mashup — are penalised.

### YouTube backends

The stream comes from `yt-dlp` when it is available, and from a configured
Invidious instance otherwise. Both are optional; with neither, Rustify falls
back to the preview clip.

## 3. Ad blocking

Spotify's audio ads are plain MP3s served from one host, and the player
marks them in the playback state. Rustify:

- **Refuses the ad hosts** before anything is downloaded, so the ad never
  reaches the speakers. The comparison is on the host alone, so an ad URL
  cannot be smuggled in as a path of an innocent host.
- **Recognises ads by their markers** — the `ad` type, a `spotify:ad:` URI,
  the `ad` provider, an advertiser, and the advertisement flag in the
  metadata — rather than by guessing from the title. A real track called
  "Advertisement" is not blocked, and a real ad is.
- **Leaves Spotify's own ad endpoints reachable** on purpose: a client that
  never calls them stands out.

## 4. Privacy enhancement

- **No analytics:** Rustify sends no telemetry headers and no analytics
  payload. The forbidden header list lives in `privacy.rs` and is covered by
  tests.
- **No logging:** Rustify writes no listening history, search queries, or
  playback events to disk.
- **No local history:** The `no_local_history` setting documents this
  explicitly.
- **Secrets are protected:** The token cache is written with owner-only
  permissions, and the `sp_dc` cookie is never written to the config file.
- **The signing secret is not hardcoded:** Rustify reads it from Spotify's
  live web player at sign-in, so a rotation does not break the login.

### Private session — what is actually true

Private session is an **account-level setting that Spotify's servers
enforce**, and it is switched on in Spotify's own desktop and mobile apps.
The public Web API has no `private_session` parameter and silently ignores
one that is sent; the web player itself ships a no-op implementation of the
feature (`isSupported: false`). There is no API to enable it for the account
and no API to read the state back — only the *active* device reports it.

Rustify therefore does not send the parameter and does not claim to enable
private session. The honest statement is:

> If private session is enabled in your Spotify app, playback started from
> Rustify is covered by it too, because the setting is enforced server-side.

## 5. Extreme Bandwidth Saver Mode

When enabled:

- **No image is ever downloaded.** The image loaders are not installed, and
  album art renders as a placeholder instead. This is the invariant that
  matters: zero image HTTP requests.
- **All animations are frozen.** egui's `animation_time` is set to zero, so
  every time-based transition — spinners, panel slides, hover fades — stops.
- **Non-essential rendering is skipped.** Rich previews and animated progress
  indicators are not drawn.

## 6. Cross-Platform

- Linux, Windows, and macOS.
- One binary per platform.
- No OpenSSL dependency: TLS is handled by rustls.
- The only system library needed on Linux is ALSA, resolved through
  `pkg-config` by `alsa-sys`.

## 7. Configuration

TOML-based config file at platform-standard location:
- Linux: `~/.config/rustify/config.toml`
- macOS: `~/Library/Application Support/Rustify/config.toml`
- Windows: `%APPDATA%/Rustify/config.toml`

See `CONFIG.md` for the full reference.
