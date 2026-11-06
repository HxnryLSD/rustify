# Rustify Features

## 1. Spotify Playback

- Browse featured playlists, new releases, and categories.
- Search tracks, albums, artists, and playlists via Spotify Web API.
- View track details, album art (unless bandwidth saver on), artist info.
- Play / pause / skip / seek playback.
- View current playback state (track, progress, duration).
- Access user library (liked songs, saved albums, followed artists).
- Create and manage playlists.

## 2. Ad Blocking (Audio, Banner, Video)

### Audio Ads
- Detects Spotify ad tracks via known ad URIs and metadata patterns.
- Skips ad tracks automatically before they reach the audio player.
- Falls back to next non-ad track in the queue/playlist.

### Banner Ads
- Blocks ad banners in the UI by filtering ad-related API responses.
- Ad placeholder areas render empty or collapsed.

### Video Ads
- Blocks video ad streams requested by the Spotify client/Web player.
- Ad video URLs matched against known ad domains are dropped.

### Configuration
All ad blocking is configurable per-type via `config.toml`:
- `block_audio_ads`
- `block_banner_ads`
- `block_video_ads`

## 3. Privacy Enhancement

- **Always Private Session:** Every Spotify API request includes `private_session=true` so listening activity is not saved to the user's profile or shared socially.
- **No Logging:** Rustify does not write listening history, search queries, or playback events to local log files.
- **Limited Telemetry:** No analytics or telemetry headers are sent to Spotify or any third party. `X-Spotify-Client-Info`-style tracking headers are stripped.
- **No Telemetry Endpoints:** No outgoing requests to analytics or telemetry services.

## 4. Extreme Bandwidth Saver Mode

When enabled:

- **Disables most image rendering:** Album art, artist images, playlist covers render as colored placeholders or text icons instead of loading images.
- **Disables all animations:** egui animations, transitions, and animated elements are frozen.
- **Only essential functions:** Non-essential UI widgets (rich previews, animated progress bars, background images) are disabled.
- **Reduced network usage:** No image HTTP requests are issued; API responses are still fetched but image URLs are not dereferenced.

## 5. Cross-Platform

- Linux, Windows, macOS.
- Single static or dynamic binary per platform.
- No external runtime dependencies beyond system audio libraries.

## 6. Configuration

TOML-based config file at platform-standard location:
- Linux: `~/.config/rustify/config.toml`
- macOS: `~/Library/Application Support/Rustify/config.toml`
- Windows: `%APPDATA%/Rustify/config.toml`

See `CONFIG.md` for full reference.