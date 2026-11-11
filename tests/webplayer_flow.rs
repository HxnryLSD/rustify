//! Integration tests for the Spotify web-player API.
//!
//! These reach the network, so they are marked `#[ignore]` and run with
//! `cargo test -- --ignored`.

use rustify::webplayer::{fetch_client_token, fetch_web_player_secret};

/// The web-player API answers a search with real tracks.
///
/// This is the mechanism that reaches the whole catalogue, including for
/// accounts without Premium.
#[tokio::test]
#[ignore = "requires network access"]
async fn the_web_player_api_searches_the_catalogue() {
    let http = reqwest::Client::new();
    let secret = fetch_web_player_secret(&http).await;
    assert!(!secret.secret_base32.is_empty());

    // Mint an anonymous web token.
    let server_time: serde_json::Value = http
        .get("https://open.spotify.com/api/server-time")
        .header("User-Agent", rustify::auth::WEB_PLAYER_USER_AGENT)
        .send()
        .await
        .expect("server time request failed")
        .json()
        .await
        .expect("server time was not JSON");
    let server_time = server_time["serverTime"]
        .as_i64()
        .expect("serverTime missing");

    let otp_local = secret.totp(rustify::totp::now_unix());
    let otp_server = secret.totp(server_time);
    let token: serde_json::Value = http
        .get("https://open.spotify.com/api/token")
        .query(&[
            ("reason", "transport"),
            ("productType", "web-player"),
            ("totp", &otp_local),
            ("totpServer", &otp_server),
            ("totpVer", &secret.version.to_string()),
        ])
        .header("User-Agent", rustify::auth::WEB_PLAYER_USER_AGENT)
        .header("Accept", "application/json")
        .header("Referer", "https://open.spotify.com/")
        .header("App-Platform", "WebPlayer")
        .send()
        .await
        .expect("token request failed")
        .json()
        .await
        .expect("token response was not JSON");

    let access_token = token["accessToken"]
        .as_str()
        .expect("no access token")
        .to_string();
    let client_id = token["clientId"]
        .as_str()
        .expect("no client id")
        .to_string();

    let client_token = fetch_client_token(&http, &client_id, "0123456789abcdef0123456789abcdef")
        .await
        .expect("the client token could not be minted");

    let client = rustify::webplayer::WebPlayerClient::new(
        rustify::webplayer::WebToken {
            access_token,
            client_id,
            expires_at: rustify::totp::now_unix() + 3600,
            sp_dc: None,
        },
        client_token,
    )
    .with_secret(secret);

    let results = client
        .search("daft punk one more time", 10)
        .await
        .expect("the search failed");

    assert!(
        !results.tracks.is_empty(),
        "the search must return tracks, got {results:?}"
    );
    assert!(
        results
            .tracks
            .iter()
            .any(|track| track.name.contains("One More Time")),
        "the expected track was not among the results: {:?}",
        results
            .tracks
            .iter()
            .map(|track| track.name.clone())
            .collect::<Vec<_>>()
    );
}

/// A public playlist can be read through the web-player API.
#[tokio::test]
#[ignore = "requires network access"]
async fn the_web_player_api_reads_a_playlist() {
    let http = reqwest::Client::new();
    let secret = fetch_web_player_secret(&http).await;

    let server_time: serde_json::Value = http
        .get("https://open.spotify.com/api/server-time")
        .header("User-Agent", rustify::auth::WEB_PLAYER_USER_AGENT)
        .send()
        .await
        .expect("server time request failed")
        .json()
        .await
        .expect("server time was not JSON");

    let otp = secret.totp(server_time["serverTime"].as_i64().unwrap_or_default());
    let token: serde_json::Value = http
        .get("https://open.spotify.com/api/token")
        .query(&[
            ("reason", "transport"),
            ("productType", "web-player"),
            ("totp", &otp),
            ("totpServer", &otp),
            ("totpVer", &secret.version.to_string()),
        ])
        .header("User-Agent", rustify::auth::WEB_PLAYER_USER_AGENT)
        .header("Accept", "application/json")
        .header("Referer", "https://open.spotify.com/")
        .header("App-Platform", "WebPlayer")
        .send()
        .await
        .expect("token request failed")
        .json()
        .await
        .expect("token response was not JSON");

    let client_id = token["clientId"].as_str().unwrap_or_default().to_string();
    let client_token = fetch_client_token(&http, &client_id, "0123456789abcdef0123456789abcdef")
        .await
        .expect("the client token could not be minted");

    let client = rustify::webplayer::WebPlayerClient::new(
        rustify::webplayer::WebToken {
            access_token: token["accessToken"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            client_id,
            expires_at: rustify::totp::now_unix() + 3600,
            sp_dc: None,
        },
        client_token,
    )
    .with_secret(secret);

    let tracks = client
        .get_playlist_contents("spotify:playlist:37i9dQZF1EIZKQBmjpvc47", 0, 5)
        .await
        .expect("the playlist could not be read");

    assert!(!tracks.is_empty(), "the playlist must have tracks");
    assert!(tracks.iter().all(|track| track.id.len() > 10));
}

/// YouTube search finds the right video for a track.
#[tokio::test]
#[ignore = "requires network access"]
async fn youtube_search_finds_the_track() {
    let source = rustify::youtube::YouTubeSource::new(rustify::youtube::YouTubeSettings::default());

    let track = rustify::models::Track {
        id: "4uLU6hMCjMI75M1A2tKUQC".into(),
        name: "One More Time".into(),
        artists: vec![rustify::models::Artist {
            id: "0gxyHStUsqpMadRV0Di1Qt".into(),
            name: "Daft Punk".into(),
            images: vec![],
            genres: vec![],
            popularity: 0,
            external_urls: Default::default(),
        }],
        album: None,
        duration_ms: 322_000,
        preview_url: None,
        track_number: None,
        disc_number: None,
        explicit: false,
        popularity: 0,
        is_local: false,
        external_urls: Default::default(),
    };

    let candidates = source
        .search(&track)
        .await
        .expect("the YouTube search failed");
    assert!(!candidates.is_empty(), "the search must return videos");

    let best = source
        .find_match(&track, &candidates)
        .expect("a video must match the track");
    assert!(
        best.title.contains("One More Time"),
        "the wrong video was chosen: {}",
        best.title
    );
}
