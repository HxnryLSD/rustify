use crate::error::{SpotifyError, SpotifyResult};
use crate::models::{Album, Artist, Playlist, Track};
use crate::totp::{self, WebPlayerSecret};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

/// The Spotify web player version this client presents itself as.
pub const CLIENT_VERSION: &str = "1.3.6.1874.g61b9bcdb1ef2";

/// The URI of the "Liked Songs" playlist. It is the same for every account.
pub const LIKED_SONGS_URI: &str = "spotify:playlist:37i9dQZF1F5p3rmiWPIYgZ";

/// Persisted-query hashes for the operations Rustify uses.
///
/// These are operation identifiers that the web player's bundles publish.
mod operation {
    pub const FETCH_PLAYLIST_CONTENTS: &str =
        "21ea11a6ac75a5f3e01fd365d29c18e0a4a8b7dc09f89bbb5d73910190c6acaf";
    pub const SEARCH_TOP_RESULTS_LIST: &str =
        "dd78eaff943eba629ed70ee25517b9cea0dcaa41193e2592ae2727660b21892c";
    pub const LIBRARY_V3: &str = "390c78e5b951029bad359785e69b07b536a509c581cbcd0aded5e5067f187455";
    pub const GET_TRACK: &str = "a8ef9e9f02b836feb0da3003c31dbb30decc6f4b473ef89ca88c882386d668de";
    pub const GET_ALBUM: &str = "11c9cad9cb24c4f9e25ef9456f68621cfd6006a90f679e729c1d2f2fd70dfe16";
    pub const QUERY_ARTIST_RELATED: &str =
        "3d031d6cb22a2aa7c8d203d49b49df731f58b1e2799cc38d9876d58771aa66f3";
    pub const GET_ARTIST_NAME_AND_TRACKS: &str =
        "0adaf1a1a8a94c7ed095639c4d9456d2b1cfac16ac511d5dd2b01b6dd89f748a";
}

/// A token issued by the web player's token endpoint.
#[derive(Debug, Clone)]
pub struct WebToken {
    /// The bearer token.
    pub access_token: String,
    /// The web player client id.
    pub client_id: String,
    /// When the token expires, in unix seconds.
    pub expires_at: i64,
    /// The `sp_dc` cookie the token belongs to, if any.
    pub sp_dc: Option<String>,
}

impl WebToken {
    /// Whether the token has expired.
    pub fn is_expired(&self) -> bool {
        totp::now_unix() >= self.expires_at
    }

    /// Whether the token expires within `margin` seconds.
    pub fn expires_soon(&self, margin: i64) -> bool {
        totp::now_unix() + margin >= self.expires_at
    }
}

/// A long-lived client token used alongside the web token.
#[derive(Debug, Clone)]
pub struct ClientToken {
    /// The token value.
    pub token: String,
    /// When it expires, in unix seconds.
    pub expires_at: i64,
}

impl ClientToken {
    /// Whether the token has expired.
    pub fn is_expired(&self) -> bool {
        totp::now_unix() >= self.expires_at
    }
}

/// The Spotify web player API.
///
/// This is the same API the browser player uses. It reaches parts of Spotify
/// that the public Web API reserves for Premium accounts, most importantly
/// the user's own library.
#[derive(Clone)]
pub struct WebPlayerClient {
    http: Arc<reqwest::Client>,
    web_token: Arc<WebToken>,
    client_token: Arc<ClientToken>,
    secret: Arc<WebPlayerSecret>,
}

impl WebPlayerClient {
    /// Build a client from a web token and a client token.
    pub fn new(web_token: WebToken, client_token: ClientToken) -> Self {
        Self {
            http: Arc::new(
                reqwest::Client::builder()
                    .timeout(Duration::from_secs(30))
                    .connect_timeout(Duration::from_secs(10))
                    // The web player's endpoints expect a browser identity.
                    .user_agent(crate::auth::WEB_PLAYER_USER_AGENT)
                    .build()
                    .unwrap_or_default(),
            ),
            web_token: Arc::new(web_token),
            client_token: Arc::new(client_token),
            secret: Arc::new(WebPlayerSecret::fallback()),
        }
    }

    /// Replace the TOTP secret, for example with the one read from the bundle.
    pub fn with_secret(mut self, secret: WebPlayerSecret) -> Self {
        self.secret = Arc::new(secret);
        self
    }

    /// The bearer token in use.
    pub fn access_token(&self) -> &str {
        &self.web_token.access_token
    }

    /// Whether the session is still usable.
    pub fn is_expired(&self) -> bool {
        self.web_token.is_expired()
    }

    /// The headers every web-player request carries.
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", self.web_token.access_token))
        {
            headers.insert(AUTHORIZATION, value);
        }
        if let Ok(value) = HeaderValue::from_str(&self.client_token.token) {
            headers.insert("client-token", value);
        }
        headers.insert("app-platform", HeaderValue::from_static("WebPlayer"));
        headers.insert(
            "spotify-app-version",
            HeaderValue::from_static(CLIENT_VERSION),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }

    /// Run a persisted-query operation against the pathfinder gateway.
    async fn pathfinder(
        &self,
        operation: &str,
        hash: &str,
        variables: Value,
    ) -> SpotifyResult<Value> {
        let body = json!({
            "operationName": operation,
            "variables": variables,
            "extensions": {
                "persistedQuery": { "version": 1, "sha256Hash": hash }
            }
        });

        let response = self
            .http
            .post("https://api-partner.spotify.com/pathfinder/v2/query")
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(SpotifyError::Status {
                code: status.as_u16(),
                body: text,
            });
        }

        let parsed: Value = serde_json::from_str(&text)?;
        if let Some(errors) = parsed.get("errors")
            && let Some(first) = errors.as_array().and_then(|items| items.first())
        {
            let message = first
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("the operation failed");
            return Err(SpotifyError::Status {
                code: 400,
                body: message.to_string(),
            });
        }
        Ok(parsed.get("data").cloned().unwrap_or(Value::Null))
    }

    /// Search the catalogue.
    ///
    /// This reaches the same index the web player's search box uses, so it
    /// works for accounts without Premium.
    pub async fn search(&self, query: &str, limit: u32) -> SpotifyResult<WebSearchResults> {
        let data = self
            .pathfinder(
                "searchTopResultsList",
                operation::SEARCH_TOP_RESULTS_LIST,
                json!({
                    "query": query,
                    "limit": limit,
                    "offset": 0,
                    "numberOfTopResults": limit,
                    "includeArtistHasConcertsField": false,
                    "includeAudiobooks": true,
                    "includeAuthors": true,
                    "includePreReleases": true,
                    "includeAlbumPreReleases": false,
                    "includeEpisodeContentRatingsV2": true,
                    "isPrefix": null,
                    "sectionFilters": ["GENERIC"]
                }),
            )
            .await?;

        Ok(WebSearchResults::parse(&data))
    }

    /// Get the tracks of a playlist.
    pub async fn get_playlist_contents(
        &self,
        uri: &str,
        offset: u32,
        limit: u32,
    ) -> SpotifyResult<Vec<Track>> {
        let data = self
            .pathfinder(
                "fetchPlaylistContents",
                operation::FETCH_PLAYLIST_CONTENTS,
                json!({
                    "uri": uri,
                    "offset": offset,
                    "limit": limit,
                    "includeEpisodeContentRatingsV2": true
                }),
            )
            .await?;

        let playlist = data.get("playlistV2").cloned().unwrap_or(Value::Null);
        if playlist.get("__typename").and_then(Value::as_str) == Some("NotFound") {
            return Err(SpotifyError::NotFound);
        }
        Ok(parse_playlist_items(&playlist))
    }

    /// Get the user's liked songs.
    ///
    /// The public Web API hides this behind Premium; the web player does not.
    pub async fn get_liked_songs(&self, limit: u32) -> SpotifyResult<Vec<Track>> {
        self.get_playlist_contents(LIKED_SONGS_URI, 0, limit).await
    }

    /// Get the user's playlists.
    pub async fn get_user_playlists(&self, limit: u32) -> SpotifyResult<Vec<Playlist>> {
        let data = self
            .pathfinder(
                "libraryV3",
                operation::LIBRARY_V3,
                json!({
                    "order": null,
                    "textFilter": "",
                    "features": ["LIKED_SONGS", "YOUR_EPISODES_V2", "PRERELEASES", "CLIPS", "EVENTS"],
                    "limit": limit,
                    "offset": 0,
                    "flatten": false,
                    "expandedFolders": [],
                    "folderUri": null,
                    "includeFoldersWhenFlattening": true
                }),
            )
            .await?;

        let mut playlists = Vec::new();
        for item in data
            .pointer("/libraryV3/items")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            if let Some(playlist) = parse_library_playlist(item) {
                playlists.push(playlist);
            }
        }
        Ok(playlists)
    }

    /// Get one track by URI.
    pub async fn get_track(&self, uri: &str) -> SpotifyResult<Track> {
        let data = self
            .pathfinder(
                "getTrack",
                operation::GET_TRACK,
                json!({ "uri": uri, "includeVideoAssociationItems": true }),
            )
            .await?;

        let track = data
            .pointer("/trackV2/data")
            .or_else(|| data.pointer("/getTrack/data"))
            .or_else(|| data.get("data"))
            .cloned()
            .unwrap_or(Value::Null);
        parse_track(&track).ok_or(SpotifyError::NotFound)
    }

    /// Get one album by URI, together with its tracks.
    pub async fn get_album(&self, uri: &str, limit: u32) -> SpotifyResult<(Album, Vec<Track>)> {
        let data = self
            .pathfinder(
                "getAlbum",
                operation::GET_ALBUM,
                json!({
                    "uri": uri,
                    "locale": "intl-en",
                    "offset": 0,
                    "limit": limit
                }),
            )
            .await?;

        let album_node = data
            .pointer("/albumOfTrack")
            .or_else(|| data.pointer("/albumV2/data"))
            .or_else(|| data.get("data"))
            .cloned()
            .unwrap_or(Value::Null);

        let album = parse_album(&album_node).ok_or(SpotifyError::NotFound)?;

        // The album's tracks come back in their own list.
        let mut tracks = Vec::new();
        for item in data
            .pointer("/albumOfTrack/trackList/items")
            .or_else(|| data.pointer("/itemsV2/items"))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let node = item.get("track").unwrap_or(item);
            if let Some(track) = parse_track(node) {
                tracks.push(track);
            }
        }

        Ok((album, tracks))
    }

    /// Get an artist's top tracks.
    pub async fn get_artist_top_tracks(&self, uri: &str, limit: u32) -> SpotifyResult<Vec<Track>> {
        let data = self
            .pathfinder(
                "getArtistNameAndTracks",
                operation::GET_ARTIST_NAME_AND_TRACKS,
                json!({ "uri": uri, "limit": limit, "offset": 0 }),
            )
            .await?;

        let mut tracks = Vec::new();
        for item in data
            .pointer("/artistDiscographyAll")
            .or_else(|| data.pointer("/topTracksV2/items"))
            .or_else(|| data.pointer("/items"))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let node = item.get("track").unwrap_or(item);
            if let Some(track) = parse_track(node) {
                tracks.push(track);
            }
        }
        Ok(tracks)
    }

    /// Get related artists.
    pub async fn get_related_artists(&self, uri: &str) -> SpotifyResult<Vec<Artist>> {
        let data = self
            .pathfinder(
                "queryArtistRelated",
                operation::QUERY_ARTIST_RELATED,
                json!({ "uri": uri }),
            )
            .await?;

        let mut artists = Vec::new();
        for item in data
            .pointer("/relatedArtists/items")
            .or_else(|| data.pointer("/items"))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let node = item.get("artist").unwrap_or(item);
            if let Some(artist) = parse_artist(node) {
                artists.push(artist);
            }
        }
        Ok(artists)
    }
}

/// The results of a web-player search.
#[derive(Debug, Clone, Default)]
pub struct WebSearchResults {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    pub playlists: Vec<Playlist>,
}

impl WebSearchResults {
    /// Parse the pathfinder response.
    ///
    /// The shape varies between result kinds, so the items are walked and
    /// classified by their URI and their `__typename`.
    pub fn parse(data: &Value) -> Self {
        let mut results = Self::default();

        let mut stack = vec![data.clone()];
        while let Some(node) = stack.pop() {
            let Some(object) = node.as_object() else {
                continue;
            };

            for (key, value) in object {
                if key == "__typename" {
                    continue;
                }
                if let Some(array) = value.as_array() {
                    for item in array {
                        stack.push(item.clone());
                    }
                    continue;
                }
                if value.is_object() {
                    stack.push(value.clone());
                    continue;
                }
            }

            // A track is recognised by its URI and a name.
            if let Some(uri) = object.get("uri").and_then(Value::as_str)
                && let Some(name) = object.get("name").and_then(Value::as_str)
                && uri.starts_with("spotify:track:")
                && let Some(track) = parse_track(&node)
            {
                let _ = name;
                if !results.tracks.iter().any(|t| t.id == track.id) {
                    results.tracks.push(track);
                }
                continue;
            }

            if let Some(uri) = object.get("uri").and_then(Value::as_str)
                && uri.starts_with("spotify:album:")
                && let Some(album) = parse_album(&node)
                && !results.albums.iter().any(|a| a.id == album.id)
            {
                results.albums.push(album);
                continue;
            }

            if let Some(uri) = object.get("uri").and_then(Value::as_str)
                && uri.starts_with("spotify:artist:")
                && let Some(artist) = parse_artist(&node)
                && !results.artists.iter().any(|a| a.id == artist.id)
            {
                results.artists.push(artist);
                continue;
            }
        }

        results
    }
}

/// Parse the items of a playlist response.
fn parse_playlist_items(playlist: &Value) -> Vec<Track> {
    let mut tracks = Vec::new();
    for item in playlist
        .pointer("/content/items")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let Some(data) = item.pointer("/itemV2/data") else {
            continue;
        };
        if let Some(track) = parse_track(data) {
            tracks.push(track);
        }
    }
    tracks
}

/// Parse a playlist entry from the library response.
fn parse_library_playlist(item: &Value) -> Option<Playlist> {
    let data = item.get("item").unwrap_or(item);
    let uri = data.get("uri").and_then(Value::as_str)?;
    if !uri.starts_with("spotify:playlist:") {
        return None;
    }
    Some(Playlist {
        id: uri.trim_start_matches("spotify:playlist:").to_string(),
        name: data
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Playlist")
            .to_string(),
        images: parse_images(data.get("images")),
        owner: None,
        description: data
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
        tracks: Default::default(),
        public: data.get("public").and_then(Value::as_bool),
        external_urls: Default::default(),
    })
}

/// Parse a track from the web-player response shape.
fn parse_track(node: &Value) -> Option<Track> {
    let uri = node.get("uri").and_then(Value::as_str)?;
    if !uri.starts_with("spotify:track:") {
        return None;
    }
    let id = uri.trim_start_matches("spotify:track:").to_string();

    // The album lives under `albumOfTrack` in this shape.
    let album_node = node
        .get("albumOfTrack")
        .or_else(|| node.get("albumOfTrack").and_then(|a| a.get("album")))
        .cloned()
        .unwrap_or(Value::Null);

    let album = if album_node.is_object() {
        Some(Album {
            id: album_node
                .get("uri")
                .and_then(Value::as_str)
                .map(|uri| uri.trim_start_matches("spotify:album:").to_string())
                .unwrap_or_default(),
            name: album_node
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            images: parse_images(
                album_node
                    .get("coverArt")
                    .or_else(|| album_node.get("images")),
            ),
            artists: parse_artists(album_node.get("artists")),
            album_type: album_node
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string),
            total_tracks: album_node
                .get("totalTrackCount")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            release_date: album_node
                .get("date")
                .and_then(Value::as_str)
                .map(str::to_string),
            genres: Vec::new(),
            external_urls: Default::default(),
        })
    } else {
        None
    };

    let artists = parse_artists(node.get("artists"));

    Some(Track {
        id,
        name: node
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        artists,
        album,
        duration_ms: node
            .get("duration")
            .and_then(|d| d.get("totalMilliseconds"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        preview_url: node
            .get("preview")
            .and_then(|p| p.get("url"))
            .and_then(Value::as_str)
            .map(str::to_string),
        track_number: node
            .get("trackNumber")
            .and_then(Value::as_u64)
            .map(|n| n as u32),
        disc_number: node
            .get("discNumber")
            .and_then(Value::as_u64)
            .map(|n| n as u32),
        explicit: node
            .get("isExplicit")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        popularity: 0,
        is_local: false,
        external_urls: Default::default(),
    })
}

/// Parse an album from the web-player response shape.
fn parse_album(node: &Value) -> Option<Album> {
    let uri = node.get("uri").and_then(Value::as_str)?;
    if !uri.starts_with("spotify:album:") {
        return None;
    }
    Some(Album {
        id: uri.trim_start_matches("spotify:album:").to_string(),
        name: node
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        images: parse_images(node.get("coverArt").or_else(|| node.get("images"))),
        artists: parse_artists(node.get("artists")),
        album_type: node.get("type").and_then(Value::as_str).map(str::to_string),
        total_tracks: node
            .get("totalTrackCount")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        release_date: node.get("date").and_then(Value::as_str).map(str::to_string),
        genres: Vec::new(),
        external_urls: Default::default(),
    })
}

/// Parse an artist from the web-player response shape.
fn parse_artist(node: &Value) -> Option<Artist> {
    let uri = node.get("uri").and_then(Value::as_str)?;
    if !uri.starts_with("spotify:artist:") {
        return None;
    }
    Some(Artist {
        id: uri.trim_start_matches("spotify:artist:").to_string(),
        name: node
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        images: parse_images(node.get("images").or_else(|| node.get("visualIdentity"))),
        genres: Vec::new(),
        popularity: 0,
        external_urls: Default::default(),
    })
}

/// Parse the artist list of a node.
fn parse_artists(node: Option<&Value>) -> Vec<Artist> {
    let Some(node) = node else {
        return Vec::new();
    };
    let items = node
        .get("items")
        .or_else(|| node.get("artists"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();

    items
        .iter()
        .filter_map(|item| {
            let profile = item.get("profile").unwrap_or(item);
            let uri = profile.get("uri").and_then(Value::as_str)?;
            Some(Artist {
                id: uri.trim_start_matches("spotify:artist:").to_string(),
                name: profile
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                images: Vec::new(),
                genres: Vec::new(),
                popularity: 0,
                external_urls: Default::default(),
            })
        })
        .collect()
}

/// Parse an image list.
fn parse_images(node: Option<&Value>) -> Vec<crate::models::Image> {
    let Some(node) = node else {
        return Vec::new();
    };
    let items = node
        .get("items")
        .or_else(|| node.get("sources"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();

    items
        .iter()
        .filter_map(|item| {
            let url = item
                .get("url")
                .and_then(Value::as_str)
                .or_else(|| item.get("uri").and_then(Value::as_str))?;
            Some(crate::models::Image {
                url: url.to_string(),
                height: item.get("height").and_then(Value::as_u64).map(|v| v as u32),
                width: item.get("width").and_then(Value::as_u64).map(|v| v as u32),
            })
        })
        .collect()
}

/// The response of the client-token endpoint.
#[derive(Debug, Deserialize)]
struct ClientTokenResponse {
    granted_token: Option<GrantedToken>,
}

/// The token itself.
#[derive(Debug, Deserialize)]
struct GrantedToken {
    token: String,
    expires_after_seconds: Option<u64>,
}

/// Fetch a client token for the given web-player client id.
///
/// The client token is valid for about two weeks and needs no sign-in.
pub async fn fetch_client_token(
    http: &reqwest::Client,
    client_id: &str,
    device_id: &str,
) -> SpotifyResult<ClientToken> {
    let body = json!({
        "client_data": {
            "client_version": CLIENT_VERSION,
            "client_id": client_id,
            "js_sdk_data": {
                "device_brand": "unknown",
                "device_model": "unknown",
                "os": std::env::consts::OS,
                "os_version": "unknown",
                "device_id": device_id,
                "device_type": "computer"
            }
        }
    });

    let response = http
        .post("https://clienttoken.spotify.com/v1/clienttoken")
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(SpotifyError::Status {
            code: status.as_u16(),
            body: text,
        });
    }

    let parsed: ClientTokenResponse = serde_json::from_str(&text)?;
    let granted = parsed.granted_token.ok_or_else(|| SpotifyError::Status {
        code: 0,
        body: "the client token response carried no token".into(),
    })?;

    Ok(ClientToken {
        token: granted.token,
        expires_at: totp::now_unix() + granted.expires_after_seconds.unwrap_or(14 * 86_400) as i64,
    })
}

/// Read the live TOTP secret from the web-player bundle.
///
/// Falls back to the bundled secret when the bundle cannot be read, so a
/// network hiccup never breaks the sign-in.
pub async fn fetch_web_player_secret(http: &reqwest::Client) -> WebPlayerSecret {
    let fallback = WebPlayerSecret::fallback();

    let Ok(page) = http
        .get("https://open.spotify.com/")
        .header("User-Agent", crate::auth::WEB_PLAYER_USER_AGENT)
        .send()
        .await
    else {
        return fallback;
    };
    let Ok(html) = page.text().await else {
        return fallback;
    };
    let Some(bundle_url) = totp::discover_bundle_url(&html) else {
        return fallback;
    };
    let Ok(bundle) = http
        .get(&bundle_url)
        .header("User-Agent", crate::auth::WEB_PLAYER_USER_AGENT)
        .send()
        .await
    else {
        return fallback;
    };
    let Ok(js) = bundle.text().await else {
        return fallback;
    };

    totp::extract_secret_from_bundle(&js).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_a_track_from_the_web_player_shape() {
        let node = json!({
            "__typename": "Track",
            "uri": "spotify:track:4uLU6hMCjMI75M1A2tKUQC",
            "name": "Never Gonna Give You Up",
            "duration": { "totalMilliseconds": 213573 },
            "discNumber": 1,
            "isExplicit": false,
            "artists": { "items": [ { "profile": { "uri": "spotify:artist:0gxyHStUsqpMadRV0Di1Qt",
                                             "name": "Rick Astley" } } ] },
            "albumOfTrack": {
                "uri": "spotify:album:6eUW0wxWtzjF9E0XMs7b1WG",
                "name": "Whenever You Need Somebody",
                "date": "1987-11-12",
                "totalTrackCount": 18,
                "coverArt": { "items": [ { "url": "https://i.scdn.co/image/abc", "width": 640, "height": 640 } ] }
            }
        });

        let track = parse_track(&node).expect("the track parses");
        assert_eq!(track.id, "4uLU6hMCjMI75M1A2tKUQC");
        assert_eq!(track.name, "Never Gonna Give You Up");
        assert_eq!(track.duration_ms, 213_573);
        assert_eq!(track.artist_names(), "Rick Astley");
        assert_eq!(track.album_name(), "Whenever You Need Somebody");
        assert_eq!(track.disc_number, Some(1));
        assert_eq!(track.album.as_ref().unwrap().images.len(), 1);
    }

    #[test]
    fn ignores_nodes_that_are_not_tracks() {
        assert!(parse_track(&json!({"uri": "spotify:artist:abc", "name": "x"})).is_none());
        assert!(parse_track(&json!({"name": "no uri"})).is_none());
    }

    #[test]
    fn parses_playlist_items() {
        let playlist = json!({
            "__typename": "Playlist",
            "content": { "items": [
                { "itemV2": { "data": { "uri": "spotify:track:aaa", "name": "One",
                                        "artists": { "items": [ { "profile": { "uri": "spotify:artist:b", "name": "B" } } ] } } } },
                { "itemV2": { "data": { "uri": "spotify:track:ccc", "name": "Two" } } },
                { "itemV2": { "data": { "uri": "spotify:episode:ddd", "name": "Episode" } } }
            ] }
        });
        let tracks = parse_playlist_items(&playlist);
        assert_eq!(tracks.len(), 2, "episodes are not tracks");
        assert_eq!(tracks[0].name, "One");
        assert_eq!(tracks[1].id, "ccc");
    }

    #[test]
    fn search_results_are_classified() {
        let data = json!({
            "sections": [ { "items": [
                { "uri": "spotify:track:t1", "name": "Track One" },
                { "uri": "spotify:album:a1", "name": "Album One" },
                { "uri": "spotify:artist:r1", "name": "Artist One" }
            ] } ]
        });
        let results = WebSearchResults::parse(&data);
        assert_eq!(results.tracks.len(), 1);
        assert_eq!(results.tracks[0].name, "Track One");
        assert_eq!(results.albums.len(), 1);
        assert_eq!(results.artists.len(), 1);
    }

    #[test]
    fn search_results_are_deduplicated() {
        let data = json!({ "a": { "uri": "spotify:track:t1", "name": "One" },
                          "b": { "uri": "spotify:track:t1", "name": "One" } });
        let results = WebSearchResults::parse(&data);
        assert_eq!(results.tracks.len(), 1);
    }

    #[test]
    fn library_playlists_are_parsed() {
        let item = json!({ "item": { "uri": "spotify:playlist:p1", "name": "My Mix",
                                     "public": true,
                                     "images": { "items": [ { "url": "https://i.scdn.co/image/x" } ] } } });
        let playlist = parse_library_playlist(&item).expect("the playlist parses");
        assert_eq!(playlist.id, "p1");
        assert_eq!(playlist.name, "My Mix");
        assert_eq!(playlist.images.len(), 1);
    }

    #[test]
    fn library_entries_that_are_not_playlists_are_skipped() {
        let item = json!({ "item": { "uri": "spotify:artist:a1", "name": "Artist" } });
        assert!(parse_library_playlist(&item).is_none());
    }
}
