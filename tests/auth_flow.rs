//! Integration tests for the authentication flows.
//!
//! The tests that reach the network are marked `#[ignore]` and run with
//! `cargo test -- --ignored`.

use rustify::auth::{AuthError, SpotifyAuth};
use rustify::totp;

/// The web player rejects a request without a valid one-time password.
///
/// A valid TOTP moves the response from `400` to `401`, which proves the
/// signature was accepted and only the cookie was refused.
#[tokio::test]
#[ignore = "requires network access"]
async fn valid_totp_is_accepted_by_the_token_endpoint() {
    let client = reqwest::Client::new();
    let user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

    let server_time: serde_json::Value = client
        .get("https://open.spotify.com/api/server-time")
        .header("User-Agent", user_agent)
        .send()
        .await
        .expect("server time request failed")
        .json()
        .await
        .expect("server time response was not JSON");
    let server_time = server_time["serverTime"]
        .as_i64()
        .expect("serverTime missing from the response");

    let secret = rustify::totp::WebPlayerSecret::fallback();
    let otp = secret.totp(server_time);
    assert_eq!(otp.len(), 6);

    let status = client
        .get("https://open.spotify.com/api/token")
        .query(&[
            ("reason", "transport"),
            ("productType", "web-player"),
            ("totp", &otp),
            ("totpServer", &otp),
            ("totpVer", &secret.version.to_string()),
        ])
        .header("Cookie", "sp_dc=definitely-not-a-valid-cookie")
        .header("User-Agent", user_agent)
        .header("Referer", "https://open.spotify.com/")
        .header("Accept", "application/json")
        .header("App-Platform", "WebPlayer")
        .send()
        .await
        .expect("token request failed")
        .status();

    assert_eq!(
        status.as_u16(),
        401,
        "a correctly signed request must be rejected for the cookie, not the signature"
    );
}

/// The live bundle carries the same secret as the bundled fallback.
#[tokio::test]
#[ignore = "requires network access"]
async fn the_live_bundle_secret_matches_the_fallback() {
    let client = reqwest::Client::new();
    let user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

    let page = client
        .get("https://open.spotify.com/")
        .header("User-Agent", user_agent)
        .send()
        .await
        .expect("the player page could not be read");
    let html = page.text().await.expect("the player page had no body");

    let bundle_url =
        rustify::totp::discover_bundle_url(&html).expect("no web-player bundle in the page");
    let bundle = client
        .get(&bundle_url)
        .header("User-Agent", user_agent)
        .send()
        .await
        .expect("the bundle could not be read");
    let js = bundle.text().await.expect("the bundle had no body");

    let live = rustify::totp::extract_secret_from_bundle(&js).expect("no secret in the bundle");
    let fallback = rustify::totp::WebPlayerSecret::fallback();

    assert_eq!(
        live.secret_base32, fallback.secret_base32,
        "the live secret must match the bundled one until Spotify rotates it"
    );
    assert_eq!(live.version, fallback.version);
}

/// A client token can be minted without signing in.
///
/// The client token is bound to the web player's client id, so the test
/// mints a web token first to learn it.
#[tokio::test]
#[ignore = "requires network access"]
async fn a_client_token_can_be_minted() {
    let client = reqwest::Client::new();
    let secret = rustify::totp::WebPlayerSecret::fallback();
    let server_time: serde_json::Value = client
        .get("https://open.spotify.com/api/server-time")
        .header("User-Agent", rustify::auth::WEB_PLAYER_USER_AGENT)
        .send()
        .await
        .expect("server time request failed")
        .json()
        .await
        .expect("server time was not JSON");

    let otp = secret.totp(server_time["serverTime"].as_i64().unwrap_or_default());
    let token: serde_json::Value = client
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

    let client_id = token["clientId"]
        .as_str()
        .expect("the web token carries no client id");

    let client_token =
        rustify::webplayer::fetch_client_token(&client, client_id, &rustify::auth::device_id())
            .await
            .expect("the client token could not be minted");
    assert!(!client_token.token.is_empty());
    assert!(client_token.expires_at > rustify::totp::now_unix());
}

/// An invalid cookie is reported as such rather than as a transport error.
#[tokio::test]
#[ignore = "requires network access"]
async fn invalid_cookie_is_reported_as_an_invalid_cookie() {
    let auth = SpotifyAuth::with_live_secret().await;
    let result = auth.sign_in("definitely-not-a-valid-cookie").await;

    match result {
        Err(AuthError::InvalidCookie) => {}
        Err(AuthError::Status { code: 401, .. }) => {}
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("an invalid cookie must not produce a token"),
    }
}

/// An empty cookie never reaches the network.
#[test]
fn empty_cookie_is_rejected_without_a_request() {
    let auth = SpotifyAuth::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(auth.sign_in("  \t "));
    assert!(matches!(result, Err(AuthError::InvalidCookie)));
}

/// The recovered secret matches the one the Spotify web player uses.
#[test]
fn web_player_secret_is_the_documented_value() {
    assert_eq!(
        totp::web_player_secret(),
        "GM3TMMJTGYZTQNZVGM4DINJZHA4TGOBYGMZTCMRTGEYDSMJRHE4TEOBUG4YTCMRUGQ4DQOJUGQYTAMRRGA2TCMJSHE3TCMBY"
    );
    assert_eq!(totp::TOTP_VERSION, 61);
}
