# Rustify

A lightweight, privacy-focused unofficial Spotify client written in Rust.

Rustify lets you browse, search, and play Spotify music without the bloat of the official client. It puts privacy and bandwidth efficiency first, with built-in ad blocking, no analytics, and an Extreme Bandwidth Saver Mode.

## Features

- 🎵 **Full catalogue** — search tracks, albums, artists, and playlists.
- 💚 **Your library, synced** — liked songs and playlists come straight from your Spotify account, on every plan.
- 🔊 **Four ways to play** — a Spotify device, YouTube, Spotify's own preview clips, or your local files.
- 🚫 **Ad blocking** — Spotify's ad audio is refused before it downloads.
- 🔒 **Privacy by default** — no analytics, no telemetry headers, no local listening history.
- 📶 **Extreme Bandwidth Saver Mode** — disables all image loading and freezes every animation.
- 🦀 **Written in Rust** — memory-safe, fast, lightweight binary.
- 🖥️ **Cross-platform** — Linux, Windows, and macOS.

## Tech Stack

- **Language:** Rust (edition 2024, MSRV 1.88)
- **GUI:** egui + eframe 0.37 (immediate-mode, OpenGL backend)
- **Audio:** rodio 0.23 (via cpal)
- **HTTP:** reqwest 0.13 with rustls
- **Async:** tokio

## Getting Started

### Prerequisites

- Rust toolchain 1.88 or newer
- A Spotify account
- Linux only: `sudo apt-get install libasound2-dev`

### Build

```bash
git clone https://github.com/HxnryLSD/rustify.git
cd rustify
cargo build --release
```

### Run

```bash
cargo run
```

## Signing in

### With the web player (recommended)

Rustify signs in the same way the Spotify web player does, so it works on **every plan, including free**. You need no developer account and no client secret.

1. Open Spotify in your browser and sign in.
2. Open the developer tools, go to **Application → Cookies → https://open.spotify.com**, and copy the `sp_dc` value.
3. Paste it into the sign-in screen and click **Sign in with cookie**.

Rustify reads Spotify's own signing secret from the live web player at sign-in, so a change on Spotify's side does not break the login.

> **Note:** this flow is not sanctioned by Spotify, and the endpoint's own error
> messages say so. It is offered for convenience. The OAuth flow below is the
> supported one.

### With OAuth

1. Create an application in the [Spotify developer dashboard](https://developer.spotify.com/dashboard).
2. Add `http://127.0.0.1:PORT/callback` to the redirect URI allowlist.
3. Paste the application's client id into the sign-in screen.

Rustify opens your browser and receives the callback on a loopback listener it starts for you. No client secret is required — this is a public client using PKCE.

> Since Spotify's February 2026 changes, registering an application requires the account owner to have Premium, which makes this path less useful on a free account.

## Where the sound comes from

Rustify has four audio sources and picks the best one available:

| Source | What it is | Needs |
|---|---|---|
| **Your device** | Playback on any Spotify Connect device, at full quality | Premium |
| **YouTube** | Rustify finds the track on YouTube and plays it locally | Nothing |
| **Preview clip** | Spotify's own ~30 second snippet | Nothing |
| **Local files** | Your own music, from a folder you choose | Nothing |

Choose one in the settings, or leave it on **Automatic**: Rustify then prefers a Spotify device, falls back to YouTube, and finally to the preview clip, so a play button is never dead.

**YouTube** is off by default. Turn it on in the settings, then give Rustify at least one way to fetch a stream:

- **`yt-dlp`** — the most reliable. Put the `yt-dlp` executable on your path or name it in the settings.
- **An Invidious instance** — for example your own. Put its address in the settings.

Rustify matches a Spotify track to a YouTube video by duration, title, artist, and channel, and rejects sped-up, remix, live, and cover versions.

## Configuration

Edit `~/.config/rustify/config.toml` (or `%APPDATA%/Rustify/config.toml` on Windows):

```toml
[privacy]
private_session = true
no_logging = true
limit_telemetry = true
no_local_history = true

[ad_blocker]
enabled = true
block_audio_ads = true
block_banner_ads = true
block_video_ads = true

[bandwidth_saver]
enabled = false
disable_images = true
disable_animations = true

[youtube]
enabled = false
invidious_instance = ""
yt_dlp_path = ""

[library]
audio_source = "automatic"
local_folder = ""
```

See `CONFIG.md` for the full reference.

## Privacy

Rustify sends only the headers the API needs — `Authorization` and `Accept`. It never sends `X-Spotify-Client-Info`, `X-Client-Id`, or any analytics payload, and it writes no logs and no listening history. Your access token is stored in a file only you can read.

**About private session:** private session is an *account-level* setting that Spotify's servers enforce, and it is switched on in Spotify's own apps. The public Web API has no parameter for it — it silently ignores one — so Rustify cannot turn it on, and it cannot read the state back for your account. What Rustify can honestly say is: **if private session is enabled in your Spotify app, playback started from Rustify is covered by it too.** Rustify never claims to guarantee it.

## Building and testing

```bash
cargo build            # debug build
cargo build --release  # optimized build
cargo test             # unit tests
cargo test -- --ignored  # tests that reach the network
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The network tests check the sign-in flow against Spotify's live servers, the web-player API, and the YouTube search.

## Roadmap

See `ROADMAP.md` for release milestones.

## License

MIT
