# Rustify — Spotify API Integration

## Authentication

Rustify uses the Spotify `sp_dc` browser cookie to obtain an access token. No Spotify Developer Dashboard registration, no `client_id`, no `client_secret`, and no OAuth Authorization Code Flow are required.

### How it works
1. User copies their `sp_dc` cookie value from their browser's Spotify cookies.
2. User pastes it into Rustify's login screen.
3. `auth.rs` (`SpotifyAuth::exchange_cookie_for_token`) POSTs the cookie to Spotify's `https://open.spotify.com/get_access_token` endpoint.
4. The response contains an `accessToken` and `accessTokenExpirationTimestampMs`, which are cached to `tokens.json`.
5. The access token is used as `Authorization: Bearer <token>` for all Spotify Web API requests.
6. When the token expires, Rustify re-exchanges the `sp_dc` cookie for a new token (the `sp_dc` flow does not issue refresh tokens).

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