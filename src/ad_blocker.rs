use crate::privacy::Privacy;
use reqwest::Request;

/// Hosts that serve Spotify's audio and video ads.
///
/// The ad *audio* files come from `adstudio-assets.scdn.co` as plain MP3s,
/// so refusing that host stops the ad from downloading at all while the
/// music keeps playing.
const AD_MEDIA_HOSTS: &[&str] = &[
    // The ad audio files.
    "adstudio-assets.scdn.co",
    // Google's ad servers (audio and video ads served in other markets).
    "pubads.g.doubleclick.net",
    "securepubads.g.doubleclick.net",
    "googleads.g.doubleclick.net",
    "pagead2.googlesyndication.com",
    "imasdk.googleapis.com",
    // The DoubleClick ad media CDN.
    "s0.2mdn.net",
];

/// Third-party audience trackers the web player loads but does not need.
const TRACKER_HOSTS: &[&str] = &[
    "googletagmanager.com",
    "google-analytics.com",
    "scorecardresearch.com",
    "fastly-insights.com",
];

/// Spotify's own ad endpoints.
///
/// These stay reachable on purpose: a client that never calls them stands
/// out, so refusing them would be worse than serving them.
const SPOTIFY_AD_HOSTS: &[&str] = &[
    "spclient.wg.spotify.com",
    "adeventtracker.spotify.com",
    "sponsored-recommendations.spotify.com",
    "api-partner.spotify.com",
    "open.spotify.com",
];

/// Ad blocker: detects and blocks audio, banner, and video ads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdBlocker {
    pub enabled: bool,
    pub block_audio_ads: bool,
    pub block_banner_ads: bool,
    pub block_video_ads: bool,
}

impl Default for AdBlocker {
    fn default() -> Self {
        Self {
            enabled: true,
            block_audio_ads: true,
            block_banner_ads: true,
            block_video_ads: true,
        }
    }
}

impl AdBlocker {
    /// Build the blocker from the persisted configuration.
    pub fn from_config(
        enabled: bool,
        block_audio_ads: bool,
        block_banner_ads: bool,
        block_video_ads: bool,
    ) -> Self {
        Self {
            enabled,
            block_audio_ads,
            block_banner_ads,
            block_video_ads,
        }
    }

    /// Whether a host serves ad media.
    ///
    /// The comparison is on the host alone, so an ad URL cannot be smuggled
    /// in as a path segment of an innocent host.
    pub fn is_ad_host(&self, host: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let host = host.to_ascii_lowercase();
        let matches = |list: &[&str]| {
            list.iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
        };
        matches(AD_MEDIA_HOSTS) || matches(TRACKER_HOSTS)
    }

    /// Whether a URL points to ad media or a tracker.
    pub fn is_ad_url(&self, url: &str) -> bool {
        if !self.enabled {
            return false;
        }
        match url::Url::parse(url) {
            Ok(parsed) => self.is_ad_host(parsed.host_str().unwrap_or_default()),
            Err(_) => self.is_ad_host(url),
        }
    }

    /// Whether a request to a Spotify-owned host must be left alone.
    ///
    /// Spotify's own ad configuration and telemetry endpoints stay reachable.
    pub fn is_spotify_ad_host(host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        SPOTIFY_AD_HOSTS
            .iter()
            .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
    }

    /// Whether a Spotify URI identifies an ad track.
    pub fn is_ad_uri(&self, uri: &str) -> bool {
        if !self.enabled || !self.block_audio_ads {
            return false;
        }
        uri.to_ascii_lowercase().starts_with("spotify:ad:")
    }

    /// Whether a queue item is an advertisement.
    ///
    /// This uses the markers Spotify actually sets on an ad rather than
    /// guessing from the title, so a real track called "Advertisement" is
    /// not blocked and a real ad is.
    ///
    /// The fields come from the player state and from the track metadata:
    /// `type` is `"ad"`, the URI is `spotify:ad:…`, the provider is `ad`,
    /// an advertiser is named, or the metadata flags it as an advertisement.
    pub fn is_ad_item(
        &self,
        item_type: Option<&str>,
        uri: Option<&str>,
        provider: Option<&str>,
        advertiser: Option<&str>,
        metadata_is_ad: Option<bool>,
    ) -> bool {
        if !self.enabled || !self.block_audio_ads {
            return false;
        }
        if item_type == Some("ad") {
            return true;
        }
        if let Some(uri) = uri
            && uri.to_ascii_lowercase().starts_with("spotify:ad:")
        {
            return true;
        }
        if provider == Some("ad") {
            return true;
        }
        if advertiser.is_some_and(|name| !name.is_empty()) {
            return true;
        }
        metadata_is_ad == Some(true)
    }

    /// Whether a track's metadata suggests it is an ad.
    ///
    /// This is the fallback for tracks that arrive without the player's own
    /// markers.
    pub fn is_ad_track(&self, track_name: &str, artist_name: &str) -> bool {
        if !self.enabled || !self.block_audio_ads {
            return false;
        }
        let tn = track_name.to_ascii_lowercase();
        let an = artist_name.to_ascii_lowercase();

        if tn.contains("advertisement") || tn.contains("sponsored") {
            return true;
        }
        // The official "Spotify" artist account serves interstitial ads.
        tn.contains("spotify") && an.contains("spotify")
    }

    /// Whether a banner or visual ad should be blocked.
    pub fn should_block_banner(&self, content_type: &str) -> bool {
        self.enabled && self.block_banner_ads && content_type.contains("advert")
    }

    /// Whether a video ad should be blocked.
    pub fn should_block_video(&self, content_type: &str) -> bool {
        self.enabled && self.block_video_ads && content_type.contains("advert")
    }

    /// Check an outgoing request against the block list.
    ///
    /// Spotify's own hosts are never refused, so the client still behaves
    /// like an ordinary player.
    pub fn should_block_request(&self, request: &Request) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(host) = request.url().host_str() else {
            return false;
        };
        if Self::is_spotify_ad_host(host) {
            return false;
        }
        self.is_ad_host(host)
    }

    /// Apply the privacy policy to an outgoing request's headers.
    pub fn apply_privacy(&self, headers: &mut reqwest::header::HeaderMap, privacy: &Privacy) {
        if privacy.is_telemetry_limited() {
            crate::privacy::strip_telemetry(headers);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_the_ad_media_host() {
        let blocker = AdBlocker::default();
        assert!(blocker.is_ad_url("https://adstudio-assets.scdn.co/mp3/abc.mp3"));
        assert!(blocker.is_ad_host("adstudio-assets.scdn.co"));
    }

    #[test]
    fn blocks_third_party_ad_servers_and_trackers() {
        let blocker = AdBlocker::default();
        assert!(blocker.is_ad_host("pubads.g.doubleclick.net"));
        assert!(blocker.is_ad_host("imasdk.googleapis.com"));
        assert!(blocker.is_ad_host("s0.2mdn.net"));
        assert!(blocker.is_ad_host("googletagmanager.com"));
        assert!(blocker.is_ad_host("scorecardresearch.com"));
    }

    #[test]
    fn leaves_spotifys_own_hosts_alone() {
        let blocker = AdBlocker::default();
        assert!(!blocker.is_ad_host("spclient.wg.spotify.com"));
        assert!(!blocker.is_ad_host("api-partner.spotify.com"));
        assert!(!blocker.is_ad_host("open.spotify.com"));
        assert!(!blocker.is_ad_host("adeventtracker.spotify.com"));
        assert!(!blocker.is_ad_host("sponsored-recommendations.spotify.com"));
    }

    #[test]
    fn ad_host_in_a_path_is_not_blocked() {
        let blocker = AdBlocker::default();
        assert!(!blocker.is_ad_url("https://api.spotify.com/adstudio-assets.scdn.co/x.mp3"));
    }

    #[test]
    fn subdomains_are_blocked_but_lookalikes_are_not() {
        let blocker = AdBlocker::default();
        assert!(blocker.is_ad_host("eu.pubads.g.doubleclick.net"));
        assert!(!blocker.is_ad_host("notdoubleclick.net"));
        assert!(!blocker.is_ad_host("evil.com"));
    }

    #[test]
    fn detects_ad_uri() {
        let blocker = AdBlocker::default();
        assert!(blocker.is_ad_uri("spotify:ad:12345"));
        assert!(!blocker.is_ad_uri("spotify:track:4iJ5W7sPJyBICUvFvt9F8F"));
    }

    #[test]
    fn detects_an_ad_by_its_markers() {
        let blocker = AdBlocker::default();
        assert!(blocker.is_ad_item(Some("ad"), None, None, None, None));
        assert!(blocker.is_ad_item(None, Some("spotify:ad:1"), None, None, None));
        assert!(blocker.is_ad_item(None, None, Some("ad"), None, None));
        assert!(blocker.is_ad_item(None, None, None, Some("Acme"), None));
        assert!(blocker.is_ad_item(None, None, None, None, Some(true)));
    }

    #[test]
    fn a_real_track_is_not_an_ad() {
        let blocker = AdBlocker::default();
        assert!(!blocker.is_ad_item(
            Some("track"),
            Some("spotify:track:abc"),
            Some("Spotify"),
            None,
            Some(false)
        ));
    }

    #[test]
    fn disabled_blocker_allows_everything() {
        let blocker = AdBlocker {
            enabled: false,
            ..Default::default()
        };
        assert!(!blocker.is_ad_url("https://adstudio-assets.scdn.co/mp3/x.mp3"));
        assert!(!blocker.is_ad_uri("spotify:ad:12345"));
        assert!(!blocker.is_ad_item(Some("ad"), None, None, None, None));
        assert!(!blocker.is_ad_track("Spotify Ad", "Spotify"));
    }

    #[test]
    fn detects_ad_track_metadata() {
        let blocker = AdBlocker::default();
        assert!(blocker.is_ad_track("Spotify Ad", "Spotify"));
        assert!(blocker.is_ad_track("Advertisement", "Sponsor"));
        assert!(!blocker.is_ad_track("Real Song", "Real Artist"));
    }

    #[test]
    fn audio_toggle_gates_track_detection() {
        let blocker = AdBlocker {
            block_audio_ads: false,
            ..Default::default()
        };
        assert!(!blocker.is_ad_track("Spotify Ad", "Spotify"));
        assert!(!blocker.is_ad_uri("spotify:ad:12345"));
    }

    #[test]
    fn request_level_blocking_works() {
        let blocker = AdBlocker::default();
        let client = reqwest::Client::new();

        let ad = client
            .get("https://adstudio-assets.scdn.co/mp3/x.mp3")
            .build()
            .unwrap();
        assert!(blocker.should_block_request(&ad));

        let tracker = client
            .get("https://www.googletagmanager.com/gtm.js")
            .build()
            .unwrap();
        assert!(blocker.should_block_request(&tracker));

        let spotify = client
            .get("https://spclient.wg.spotify.com/ads/v2/config")
            .build()
            .unwrap();
        assert!(!blocker.should_block_request(&spotify));

        let ok = client.get("https://api.spotify.com/v1/me").build().unwrap();
        assert!(!blocker.should_block_request(&ok));
    }
}
