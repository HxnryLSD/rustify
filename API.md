# Rustify — Spotify API Integration

## Authentication

Rustify uses OAuth 2.0 Authorization Code Flow with PKCE (recommended for public clients).

### Scopes requested
- `user-read-private` — read user profile
- `user-read-email` — read user email
- `user-library-read` — read saved tracks/albums
- `user-library-modify` — save/remove tracks/albums
- `playlist-read-private` — read private playlists
- `playlist-modify-private` — modify private playlists
- `user-read-playback-state` — read current playback
- `user-modify-playback-state` — control playback
- `user-read-currently-playing` — read currently playing track
- `streaming` — enable playback via Web Playback SDK (conceptual; Rustify uses its own audio path)

### Flow
1. User clicks "Log in" → egui opens system browser at Spotify accounts authorization URL.
2. User grants permission → Spotify redirects to `http://localhost:8888/callback` with `code`.
3. Rustify's local callback listener captures `code`.
4. `auth.rs` exchanges `code` for `access_token` + `refresh_token`.
5. Tokens saved to `tokens.json`; access token used for all API requests.
6. On 401 (expired), `auth.rs` refreshes using `refresh_token`.

## Spotify Web API Endpoints Used

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/me` | GET | Current user profile |
| `/v1/me/player` | GET | Current playback state |
| `/v1/me/player/play` | PUT | Start/resume playback |
| `/v1/me/player/pause` | PUT | Pause playback |
| `/v1/me/player/next` | POST | Skip to next track |
| `/v1/me/player/previous` | POST | Skip to previous track |
| `/v1/me/player/seek` | PUT | Seek to position |
| `/v1/search` | GET | Search tracks/albums/artists/playlists |
| `/v1/tracks/{id}` | GET | Track details |
| `/v1/albums/{id}` | GET | Album details |
| `/v1/artists/{id}` | GET | Artist details |
| `/v1/playlists/{id}` | GET | Playlist details |
| `/v1/me/tracks` | GET | Saved tracks (library) |
| `/v1/me/tracks` | PUT/DELETE | Save/remove track |
| `/v1/browse/featured-playlists` | GET | Featured playlists |
| `/v1/browse/new-releases` | GET | New releases |
| `/v1/browse/categories` | GET | Browse categories |

## Rate Limiting

Spotify Web API rate limits are handled by `reqwest` retry logic + exponential backoff in `spotify_client.rs`. 429 responses are retried after the `Retry-After` header duration.

## Privacy Headers

All requests include:
- `Authorization: Bearer {access_token}`
- `Accept: application/json`

The client does NOT send:
- `X-Spotify-Client-Info` or similar telemetry headers.
- Any analytics payload.

`private_session=true` is appended to playback control requests where supported.