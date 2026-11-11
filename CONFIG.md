# Rustify Configuration

## Config File Location

- Linux: `~/.config/rustify/config.toml`
- macOS: `~/Library/Application Support/Rustify/config.toml`
- Windows: `%APPDATA%/Rustify/config.toml`

`RUSTIFY_CONFIG_PATH` overrides the location of the whole configuration
directory. When it is set, the token cache becomes
`<RUSTIFY_CONFIG_PATH with a .tokens.json extension>`.

## Config Format (TOML)

```toml
[auth]
# Spotify application client id, used by the OAuth PKCE flow.
# Leave empty to use the web-player cookie sign-in instead.
client_id = ""

[privacy]
# Report the private-session state in the interface.
#
# Private session is an account-level setting that Spotify's servers enforce
# and that only Spotify's own apps can switch on. The Web API ignores any
# parameter sent for it, so this only controls what the interface says.
private_session = true
# Disable all local logging
no_logging = true
# Limit telemetry and analytics headers
limit_telemetry = true
# Do not persist listening history locally
no_local_history = true

[ad_blocker]
# Enable ad blocking globally
enabled = true
# Block audio ads (ad tracks skipped before playback)
block_audio_ads = true
# Block banner ads in UI
block_banner_ads = true
# Block video ads
block_video_ads = true

[bandwidth_saver]
# Enable Extreme Bandwidth Saver Mode
enabled = false
# Disable most image rendering (album art, artist images, covers)
disable_images = true
# Disable all animations
disable_animations = true

[audio]
# Preferred output device (empty = system default)
output_device = ""
# Volume (0.0 to 1.0)
volume = 0.8
# Prebuffer size in seconds for audio streams
prebuffer_seconds = 8.0

[youtube]
# Use YouTube as an audio source
enabled = false
# An Invidious instance, for example https://inv.example.org
invidious_instance = ""
# The yt-dlp executable, for example /usr/bin/yt-dlp
yt_dlp_path = ""

[library]
# Where the audio comes from:
#   automatic  - a Spotify device, then YouTube, then the preview clip
#   device     - always play on a Spotify Connect device
#   preview    - always play Spotify's preview clip
#   youtube    - always play from YouTube
#   local      - play from the local music folder
audio_source = "automatic"
# The folder scanned for local music
local_folder = ""

[ui]
# Window width
window_width = 1180.0
# Window height
window_height = 760.0
# Theme: "light" or "dark"
theme = "dark"
# Show the album art panel on the right
show_cover_panel = true
# Number of search results to request per query
search_limit = 30
```

## Defaults

Every field has a default, so a missing or partial file never discards your
settings. Each section is parsed independently of the others: an unknown or
malformed value falls back to its default rather than resetting the file.

If the file cannot be parsed at all, Rustify keeps a copy at
`config.toml.broken`, starts from the defaults, and reports the problem on
stderr.

The file is created on first launch with the values above.

## Token Cache

The access and refresh tokens are stored separately in `tokens.json`, next to
the config file:

- Linux: `~/.config/rustify/tokens.json`
- macOS: `~/Library/Application Support/Rustify/tokens.json`
- Windows: `%APPDATA%/Rustify/tokens.json`

Both files are written with owner-only permissions (`0600` on Unix).

The `sp_dc` cookie is never written to `config.toml`.

## Environment Variables

- `RUSTIFY_CONFIG_PATH` — override the location of the configuration directory.
