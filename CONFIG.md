# Rustify Configuration

## Config File Location

- Linux: `~/.config/rustify/config.toml`
- macOS: `~/Library/Application Support/Rustify/config.toml`
- Windows: `%APPDATA%/Rustify/config.toml`

## Config Format (TOML)

```toml
[privacy]
# Force private session on all Spotify API calls (always true, enforced)
private_session = true
# Disable all local logging
no_logging = true
# Limit telemetry and analytics headers
limit_telemetry = true

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

[ui]
# Window width
window_width = 1024
# Window height
window_height = 768
# Theme: "light" or "dark"
theme = "dark"
```

## Defaults

All fields default to the values shown above if the config file is missing or incomplete. On first launch, Rustify creates the config file with defaults.

## Environment Variables

- `RUSTIFY_CONFIG_PATH` — override config file location.
- `RUSTIFY_LOG_LEVEL` — log level (only used if `no_logging = false`; defaults to off).