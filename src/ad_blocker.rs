use std::sync::OnceLock;

/// Known Spotify ad-serving domains to block.
pub static AD_DOMAINS: OnceLock<Vec<&'static str>> = OnceLock::new();

fn default_ad_domains() -> Vec<&'static str> {
    vec![
        "adclick.g.doubleclick.net",
        "googleads.g.doubleclick.net",
        "pubads.g.doubleclick.net",
        "securepubads.g.doubleclick.net",
        "spotify-ad-core.scdn.co",
        "spotify-ad-fetch.scdn.co",
        "spotify-ads.scdn.co",
        "audio-ads.scdn.co",
        "video-ads.scdn.co",
    ]
}

/// Known URI prefixes that identify ad tracks/playlists.
pub static AD_URI_PREFIXES: OnceLock<Vec<&'static str>> = OnceLock::new();

fn default_ad_uri_prefixes() -> Vec<&'static str> {
    vec![
        "spotify:ad:",
        "spotify:track:ad",
        "spotify:track:spotify-ad",
    ]
}

/// Ad blocker: detects and blocks audio, banner, and video ads.
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

    /// Check whether a URL points to a known ad domain.
    pub fn is_ad_url(&self, url: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let lower = url.to_lowercase();
        AD_DOMAINS
            .get_or_init(default_ad_domains)
            .iter()
            .any(|d| lower.contains(d))
    }

    /// Check whether a Spotify URI identifies an ad track.
    pub fn is_ad_uri(&self, uri: &str) -> bool {
        if !self.enabled || !self.block_audio_ads {
            return false;
        }
        let lower = uri.to_lowercase();
        AD_URI_PREFIXES
            .get_or_init(default_ad_uri_prefixes)
            .iter()
            .any(|p| lower.starts_with(p))
    }

    /// Check whether a track's metadata suggests it's an ad.
    pub fn is_ad_track(&self, track_name: &str, artist_name: &str) -> bool {
        if !self.enabled || !self.block_audio_ads {
            return false;
        }
        let tn = track_name.to_lowercase();
        let an = artist_name.to_lowercase();
        (tn.contains("spotify") && an.contains("spotify"))
            || tn.contains("advertisement")
            || tn.contains("sponsored")
    }

    /// Determine if a banner/visual ad should be blocked.
    pub fn should_block_banner(&self, _content_type: &str) -> bool {
        self.enabled && self.block_banner_ads
    }

    /// Determine if a video ad should be blocked.
    pub fn should_block_video(&self, _content_type: &str) -> bool {
        self.enabled && self.block_video_ads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_known_ad_domain() {
        let blocker = AdBlocker::default();
        assert!(blocker.is_ad_url("https://pubads.g.doubleclick.net/gampad/ads"));
        assert!(!blocker.is_ad_url("https://api.spotify.com/v1/tracks/123"));
    }

    #[test]
    fn detects_ad_uri() {
        let blocker = AdBlocker::default();
        assert!(blocker.is_ad_uri("spotify:ad:12345"));
        assert!(!blocker.is_ad_uri("spotify:track:4iJ5W7sPJyBICUvFvt9F8F"));
    }

    #[test]
    fn disabled_blocker_allows_everything() {
        let blocker = AdBlocker {
            enabled: false,
            ..Default::default()
        };
        assert!(!blocker.is_ad_url("https://pubads.g.doubleclick.net/gampad/ads"));
        assert!(!blocker.is_ad_uri("spotify:ad:12345"));
    }
}
