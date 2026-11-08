# Rustify

A lightweight unofficial Spotify client written fully in Rust.

Rustify lets you browse, search, and play Spotify music without the bloat of the official client. It puts privacy and bandwidth efficiency first, with built-in ad blocking, always-private sessions, and an Extreme Bandwidth Saver Mode.

## Features

- 🎵 **Spotify playback** — browse, search, and play tracks, albums, and playlists via the Spotify Web API.
- 🚫 **Ad blocking** — blocks audio, banner, and video ads at the stream and request level.
- 🔒 **Privacy enhancement** — always private session, no logging, limited telemetry and analytics.
- 📶 **Extreme Bandwidth Saver Mode** — disables most image rendering, all animations, and non-essential functions to save bandwidth.
- 🦀 **Written in Rust** — memory-safe, fast, lightweight binary.
- 🖥️ **Cross-platform** — Linux, Windows, and macOS.

## Tech Stack

- **Language:** Rust
- **GUI:** egui + eframe (immediate-mode, lightweight)
- **Audio:** rodio (cross-platform playback)
- **HTTP:** reqwest
- **Auth:** sp_dc cookie exchange (no client_id/secret needed)
- **Async:** tokio

## Getting Started

### Prerequisites

- Rust toolchain (1.75+)
- A Spotify account
- Spotify Developer application (for OAuth credentials)

### Build

```bash
git clone https://github.com/your-org/rustify.git
cd rustify
cargo build --release
```

### Run

```bash
cargo run
```

On first launch, Rustify opens a browser window for Spotify OAuth login. After authorizing, you're redirected back to Rustify and can start browsing and playing music.

## Configuration

Edit `~/.config/rustify/config.toml` (or `%APPDATA%/Rustify/config.toml` on Windows) to adjust:

```toml
[privacy]
private_session = true
no_logging = true
limit_telemetry = true

[ad_blocker]
enabled = true
block_audio_ads = true
block_banner_ads = true
block_video_ads = true

[bandwidth_saver]
enabled = false
disable_images = true
disable_animations = true
```

See `CONFIG.md` for the full reference.

## Roadmap

See `ROADMAP.md` for release milestones.

## License

MIT