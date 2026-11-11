# Rustify — Spotify API Integration

Rustify talks to two different Spotify surfaces.

## The public Web API — `api.spotify.com`

The documented API. It covers the catalogue, playback control, and the
library, but Spotify reserves **playback control** and **your library** for
Premium accounts:

| Endpoint | Free account |
|---|---|
| `GET /v1/search`, catalogue reads | works |
| `PUT /v1/me/player/play` and the other player endpoints | `403` |
| `GET /v1/me/tracks` (liked songs) | `403` |

That is why Rustify also speaks the web player's API.

## The web-player API

The same API the browser player uses. It is not documented, and Spotify can
change it, but it reaches everything the public API hides.

### Signing in

```
GET https://open.spotify.com/api/token
    ?reason=transport            (or init)
    &productType=web-player
    &totp={code}                 6 digits from the local clock
    &totpServer={code}           6 digits from Spotify's server clock
    &totpVer={version}
```

with the `sp_dc` cookie, a browser `User-Agent`, and
`App-Platform: WebPlayer`.

The older `https://open.spotify.com/get_access_token` path is blocked at the
CDN edge and answers `403 URL Blocked`.

**The one-time password.** The web player derives it from a versioned secret
embedded in its JavaScript bundle. Rustify reads the live bundle at sign-in
(`open.spotify.com` → the `web-player.<hash>.js` file → `{secret:'…',version:N}`)
and falls back to a bundled copy, so a rotation does not break the login.

The secret is unmasked with `value ^ ((index % 33) + 9)`, the result is
rendered in decimal, and that text is base32-encoded to give the HMAC key.

**The server clock** comes from `GET https://open.spotify.com/api/server-time`,
which answers `{"serverTime": <unix seconds>}`.

**The User-Agent matters.** The web-player endpoints answer
`403 Client/request not allowed` to a request that does not look like a
browser. Rustify sends the same `User-Agent` the web player does.

### The client token

A second token, valid for about two weeks and needed by every catalogue
request:

```
POST https://clienttoken.spotify.com/v1/clienttoken
Content-Type: application/json

{"client_data": {
   "client_version": "1.3.6.1874.g61b9bcdb1ef2",
   "client_id": "<the client id from the token above>",
   "js_sdk_data": {"device_brand": "unknown", "device_model": "unknown",
                   "os": "linux", "os_version": "unknown",
                   "device_id": "<a 36 character id>", "device_type": "computer"}}}
```

The answer carries `granted_token.token`, which is sent as the `client-token`
header.

### The pathfinder gateway

Catalogue and library reads go through a GraphQL-style gateway:

```
POST https://api-partner.spotify.com/pathfinder/v2/query

{"operationName": "fetchPlaylistContents",
 "variables": {...},
 "extensions": {"persistedQuery": {"version": 1, "sha256Hash": "..."}}}
```

The `sha256Hash` values are the operation identifiers the web player's
bundles publish. Rustify uses the ones in `src/webplayer.rs`.

Requests carry these headers:

| Header | Value |
|---|---|
| `authorization` | `Bearer {web token}` |
| `client-token` | `{client token}` |
| `app-platform` | `WebPlayer` |
| `spotify-app-version` | `1.3.6.1874.g61b9bcdb1ef2` |
| `accept` | `application/json` |

### Operations used

| Operation | Purpose |
|---|---|
| `fetchPlaylistContents` | The tracks of a playlist, including **Liked Songs** |
| `searchTopResultsList` | The search behind the web player's search box |
| `libraryV3` | The user's library, with their playlists |
| `getTrack` | One track |
| `getAlbum` | One album and its tracks |
| `getArtistNameAndTracks` | An artist's top tracks |
| `queryArtistRelated` | Related artists |

The Liked Songs playlist has the same URI for every account:
`spotify:playlist:37i9dQZF1F5p3rmiWPIYgZ`.

## The public Web API endpoints used

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/me` | GET | Current user profile |
| `/v1/me/player` | GET | Current playback state |
| `/v1/me/player/devices` | GET | Available playback devices |
| `/v1/me/player/play` | PUT | Start/resume playback |
| `/v1/me/player/pause` | PUT | Pause playback |
| `/v1/me/player/next` | POST | Skip to next track |
| `/v1/me/player/previous` | POST | Skip to previous track |
| `/v1/me/player/seek` | PUT | Seek to position |
| `/v1/me/player/volume` | PUT | Set the device volume |
| `/v1/me/player/shuffle` | PUT | Toggle shuffle |
| `/v1/me/player/repeat` | PUT | Set the repeat mode |
| `/v1/me/player` | PUT | Transfer playback to another device |
| `/v1/me/player/recently-played` | GET | Recently played tracks |
| `/v1/search` | GET | Search tracks/albums/artists/playlists |
| `/v1/tracks/{id}` | GET | Track details |
| `/v1/albums/{id}` | GET | Album details |
| `/v1/albums/{id}/tracks` | GET | Album tracks |
| `/v1/artists/{id}` | GET | Artist details |
| `/v1/artists/{id}/top-tracks` | GET | Artist top tracks |
| `/v1/artists/{id}/albums` | GET | Artist albums |
| `/v1/artists/{id}/related-artists` | GET | Related artists |
| `/v1/playlists/{id}` | GET | Playlist details |
| `/v1/playlists/{id}/tracks` | GET | Playlist tracks |
| `/v1/me/tracks` | GET | Saved tracks |
| `/v1/me/tracks` | PUT/DELETE | Save/remove track |
| `/v1/me/albums` | GET | Saved albums |
| `/v1/me/following` | GET | Followed artists |
| `/v1/me/playlists` | GET | The user's playlists |
| `/v1/browse/featured-playlists` | GET | Featured playlists |
| `/v1/browse/new-releases` | GET | New releases |
| `/v1/browse/categories` | GET | Browse categories |

## Rate limiting

`spotify_client.rs` retries `429 Too Many Requests` responses up to three
times, honouring the `Retry-After` header (capped at 30 seconds). `5xx`
responses are retried with an exponential backoff. When the retries are
exhausted the client returns a `RateLimited` error, which the interface
reports to the user.

## Privacy headers

All requests include:
- `Authorization: Bearer {access_token}`
- `Accept: application/json`

The client does NOT send:
- `X-Spotify-Client-Info` or similar telemetry headers.
- Any analytics payload.

The list of headers that are stripped is kept in `privacy.rs` as
`FORBIDDEN_HEADERS` so it can be audited and tested.

## Ads

Spotify serves its audio ads from `adstudio-assets.scdn.co` as plain MP3s,
and inserts them into the playback state machine one track ahead of time. The
player marks them with `type: "ad"`, a `spotify:ad:` URI, an advertiser, and
a `disallow_seeking` flag.

Rustify refuses the ad hosts before anything is downloaded, and recognises
ads by those markers rather than by guessing from the title.

Spotify's own ad endpoints stay reachable on purpose: a client that never
calls them stands out.

## YouTube

The YouTube source searches with the YouTube web player's own API, which
needs no key of its own:

```
POST https://www.youtube.com/youtubei/v1/search?key=...
```

The stream itself comes from `yt-dlp` or from an Invidious instance, because
YouTube's playback endpoints refuse requests that look automated and change
their anti-bot measures regularly.

## About `private_session`

The public Web API has no `private_session` parameter, and it silently
ignores one that is sent — `private_session=true` is not a Web API feature.
Private session is an account-level setting that Spotify's servers enforce
and that only Spotify's own apps can switch on.

Rustify therefore does not send the parameter and does not claim to enable
private session. If private session is on in your Spotify app, playback
started from Rustify is covered by it, because the setting is enforced
server-side.
