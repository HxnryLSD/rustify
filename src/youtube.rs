use crate::models::Track;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;

/// The public API key the YouTube web player uses.
///
/// It is not a secret: it ships in the web player itself and only works
/// together with a client version and the matching context.
const INNERTUBE_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";

/// The web client identity used for search requests.
const CLIENT_NAME: &str = "WEB";
const CLIENT_VERSION: &str = "2.20240709.01.00";

/// How close a candidate's duration has to be, as a fraction.
const DURATION_TOLERANCE: f64 = 0.08;

/// A video found on YouTube.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YouTubeVideo {
    /// The video id.
    pub id: String,
    /// The video title.
    pub title: String,
    /// The channel that published it.
    pub channel: String,
    /// The duration in seconds.
    pub duration_seconds: Option<u64>,
    /// The page URL.
    pub url: String,
}

/// A playable audio stream.
#[derive(Debug, Clone)]
pub struct AudioStream {
    /// The direct URL of the audio.
    pub url: String,
    /// The MIME type of the stream.
    pub mime_type: String,
    /// The bitrate in bits per second, when known.
    pub bitrate: Option<u64>,
}

/// Where a stream came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamBackend {
    /// The `yt-dlp` helper, which handles YouTube's anti-bot measures.
    YtDlp,
    /// An Invidious instance.
    Invidious,
}

impl StreamBackend {
    /// A short name for the interface.
    pub fn label(self) -> &'static str {
        match self {
            StreamBackend::YtDlp => "yt-dlp",
            StreamBackend::Invidious => "Invidious",
        }
    }
}

/// How the YouTube audio source should behave.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct YouTubeSettings {
    /// Whether YouTube may be used as an audio source at all.
    pub enabled: bool,
    /// An Invidious instance to use, when the user configured one.
    pub invidious_instance: Option<String>,
    /// The `yt-dlp` executable to run.
    pub yt_dlp_path: Option<PathBuf>,
}

/// The YouTube audio source.
///
/// It searches YouTube for a track, scores the candidates, and resolves one
/// of them to a playable stream.
#[derive(Clone)]
pub struct YouTubeSource {
    http: reqwest::Client,
    settings: YouTubeSettings,
}

impl YouTubeSource {
    /// Build a source with the given settings.
    pub fn new(settings: YouTubeSettings) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent(concat!("Rustify/", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_default(),
            settings,
        }
    }

    /// The settings in use.
    pub fn settings(&self) -> &YouTubeSettings {
        &self.settings
    }

    /// Search YouTube for a track.
    pub async fn search(&self, track: &Track) -> Result<Vec<YouTubeVideo>, String> {
        let query = format!("{} {}", track.artist_name(), track.name);
        self.search_query(&query).await
    }

    /// Search YouTube for a free-text query.
    pub async fn search_query(&self, query: &str) -> Result<Vec<YouTubeVideo>, String> {
        let body = json!({
            "context": { "client": { "clientName": CLIENT_NAME, "clientVersion": CLIENT_VERSION, "hl": "en" } },
            "query": query
        });

        let response = self
            .http
            .post(format!(
                "https://www.youtube.com/youtubei/v1/search?key={INNERTUBE_KEY}"
            ))
            .header("Content-Type", "application/json")
            .header("X-YouTube-Client-Name", "1")
            .header("X-YouTube-Client-Version", CLIENT_VERSION)
            .header("Origin", "https://www.youtube.com")
            .json(&body)
            .send()
            .await
            .map_err(|err| format!("YouTube search failed: {err}"))?;

        let text = response
            .text()
            .await
            .map_err(|err| format!("YouTube search failed: {err}"))?;
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|err| format!("YouTube reply was not JSON: {err}"))?;

        Ok(parse_search_results(&parsed))
    }

    /// Find the best matching video for a track.
    pub fn find_match(&self, track: &Track, candidates: &[YouTubeVideo]) -> Option<YouTubeVideo> {
        candidates
            .iter()
            .filter_map(|video| score(video, track).map(|score| (score, video.clone())))
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, video)| video)
    }

    /// Resolve a video to a playable audio stream.
    ///
    /// The backends are tried in order: `yt-dlp` first because it copes
    /// with YouTube's anti-bot measures, then a configured Invidious
    /// instance.
    pub async fn resolve_stream(&self, video: &YouTubeVideo) -> Result<AudioStream, String> {
        let mut last_error = String::new();

        if let Some(path) = &self.settings.yt_dlp_path {
            match resolve_with_ytdlp(path, video).await {
                Ok(stream) => return Ok(stream),
                Err(err) => last_error = err,
            }
        }

        if let Some(instance) = &self.settings.invidious_instance {
            match resolve_with_invidious(&self.http, instance, video).await {
                Ok(stream) => return Ok(stream),
                Err(err) => last_error = err,
            }
        }

        if last_error.is_empty() {
            last_error = "no YouTube backend is configured".to_string();
        }
        Err(last_error)
    }
}

/// Parse the InnerTube search response.
fn parse_search_results(data: &Value) -> Vec<YouTubeVideo> {
    let mut videos = Vec::new();
    let mut stack = vec![data.clone()];

    while let Some(node) = stack.pop() {
        let Some(object) = node.as_object() else {
            continue;
        };
        for (key, value) in object {
            if key == "videoRenderer" {
                if let Some(video) = parse_video_renderer(value) {
                    videos.push(video);
                }
                continue;
            }
            if let Some(array) = value.as_array() {
                for item in array {
                    stack.push(item.clone());
                }
            } else if value.is_object() {
                stack.push(value.clone());
            }
        }
    }

    videos
}

/// Parse one `videoRenderer` entry.
fn parse_video_renderer(node: &Value) -> Option<YouTubeVideo> {
    let id = node.get("videoId")?.as_str()?.to_string();
    let title = node
        .pointer("/title/runs/0/text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let channel = node
        .pointer("/ownerText/runs/0/text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let duration_seconds = node
        .get("lengthSeconds")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok());

    Some(YouTubeVideo {
        url: format!("https://www.youtube.com/watch?v={id}"),
        id,
        title,
        channel,
        duration_seconds,
    })
}

/// Score how well a candidate matches a track.
///
/// The duration is the strongest signal: a live cover or a sped-up edit has
/// a different length, and so does a one-hour loop.
pub fn score(video: &YouTubeVideo, track: &Track) -> Option<f64> {
    if track.duration_ms == 0 {
        return None;
    }
    let track_seconds = track.duration_ms as f64 / 1000.0;

    let mut score = 0.0;

    // Duration: the closer, the better. A mismatch beyond the tolerance
    // rules the candidate out.
    match video.duration_seconds {
        Some(seconds) if seconds > 0 => {
            let difference = (seconds as f64 - track_seconds).abs() / track_seconds;
            if difference > DURATION_TOLERANCE {
                return None;
            }
            score += 100.0 * (1.0 - difference / DURATION_TOLERANCE);
        }
        _ => {
            // No duration to compare, so the candidate is only a guess.
            score -= 20.0;
        }
    }

    // Title and artist text.
    let title = video.title.to_lowercase();
    let artist = track.artist_name().to_lowercase();
    let name = track.name.to_lowercase();

    if !artist.is_empty() && title.contains(&artist) {
        score += 40.0;
    }
    if !name.is_empty() && title.contains(&name) {
        score += 40.0;
    }

    // The artist's own channel is a good sign.
    let channel = video.channel.to_lowercase();
    if !artist.is_empty() && !channel.is_empty() && channel == artist {
        score += 30.0;
    } else if !artist.is_empty() && channel.contains(&artist) {
        score += 15.0;
    }

    // Versions that are not the studio recording.
    for marker in [
        "sped up", "remix", "live", "cover", "karaoke", "mashup", "8d",
    ] {
        if title.contains(marker) {
            score -= 35.0;
        }
    }

    Some(score)
}

/// Resolve a video with the `yt-dlp` helper.
async fn resolve_with_ytdlp(path: &PathBuf, video: &YouTubeVideo) -> Result<AudioStream, String> {
    let output = tokio::process::Command::new(path)
        .args([
            "--no-playlist",
            "--no-warnings",
            "-f",
            "bestaudio",
            "-g",
            &video.url,
        ])
        .output()
        .await
        .map_err(|err| format!("yt-dlp could not be run: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "yt-dlp failed: {}",
            stderr.trim().chars().take(200).collect::<String>()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let url = stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("http"))
        .ok_or_else(|| "yt-dlp returned no stream URL".to_string())?;

    Ok(AudioStream {
        url: url.to_string(),
        mime_type: "audio/mp4".to_string(),
        bitrate: None,
    })
}

/// The Invidious video response.
#[derive(Debug, Deserialize)]
struct InvidiousVideo {
    adaptive_formats: Option<Vec<InvidiousFormat>>,
}

/// One Invidious stream format.
#[derive(Debug, Deserialize)]
struct InvidiousFormat {
    url: Option<String>,
    mime_type: Option<String>,
    bitrate: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

/// Resolve a video with an Invidious instance.
async fn resolve_with_invidious(
    http: &reqwest::Client,
    instance: &str,
    video: &YouTubeVideo,
) -> Result<AudioStream, String> {
    let url = format!(
        "{}/api/v1/videos/{}",
        instance.trim_end_matches('/'),
        video.id
    );
    let response = http
        .get(&url)
        .send()
        .await
        .map_err(|err| format!("Invidious request failed: {err}"))?;

    let text = response
        .text()
        .await
        .map_err(|err| format!("Invidious reply failed: {err}"))?;
    let parsed: InvidiousVideo = serde_json::from_str(&text)
        .map_err(|err| format!("Invidious reply was not JSON: {err}"))?;

    let formats = parsed.adaptive_formats.unwrap_or_default();
    formats
        .iter()
        .filter(|format| format.kind.as_deref() == Some("audio"))
        .filter_map(|format| {
            let url = format.url.clone()?;
            Some(AudioStream {
                url,
                mime_type: format.mime_type.clone().unwrap_or_default(),
                bitrate: format.bitrate.as_ref().and_then(|value| value.parse().ok()),
            })
        })
        .max_by_key(|stream| stream.bitrate.unwrap_or(0))
        .ok_or_else(|| "Invidious returned no audio stream".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Artist;

    fn track(name: &str, artist: &str, duration_ms: u32) -> Track {
        Track {
            id: "abc".into(),
            name: name.into(),
            artists: vec![Artist {
                id: "ar".into(),
                name: artist.into(),
                images: vec![],
                genres: vec![],
                popularity: 0,
                external_urls: Default::default(),
            }],
            album: None,
            duration_ms,
            preview_url: None,
            track_number: None,
            disc_number: None,
            explicit: false,
            popularity: 0,
            is_local: false,
            external_urls: Default::default(),
        }
    }

    fn video(title: &str, channel: &str, seconds: u64) -> YouTubeVideo {
        YouTubeVideo {
            id: "v1".into(),
            title: title.into(),
            channel: channel.into(),
            duration_seconds: Some(seconds),
            url: "https://www.youtube.com/watch?v=v1".into(),
        }
    }

    #[test]
    fn the_official_video_wins() {
        let t = track("One More Time", "Daft Punk", 322_000);
        let official = video(
            "Daft Punk - One More Time (Official Video)",
            "Daft Punk",
            322,
        );
        let radio = video("One More Time (Radio Edit)", "Daft Punk", 236);
        let essay = video("How Daft Punk made One more time", "Grumpy Michi", 44);

        let best = [official, radio, essay]
            .iter()
            .filter_map(|v| score(v, &t).map(|s| (s, v.clone())))
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .unwrap()
            .1;
        assert_eq!(best.title, "Daft Punk - One More Time (Official Video)");
    }

    #[test]
    fn a_wrong_length_is_rejected() {
        let t = track("One More Time", "Daft Punk", 322_000);
        assert_eq!(score(&video("One More Time", "Daft Punk", 60), &t), None);
        assert_eq!(score(&video("One More Time", "Daft Punk", 600), &t), None);
    }

    #[test]
    fn a_cover_is_penalised() {
        let t = track("One More Time", "Daft Punk", 322_000);
        let studio = video("Daft Punk - One More Time", "Daft Punk", 322);
        let cover = video("One More Time (Acme Cover)", "Acme", 322);
        assert!(score(&studio, &t).unwrap() > score(&cover, &t).unwrap());
    }

    #[test]
    fn a_sped_up_version_is_rejected() {
        let t = track("One More Time", "Daft Punk", 322_000);
        let normal = video("Daft Punk - One More Time", "Daft Punk", 322);
        let sped = video("Daft Punk - One More Time (Sped Up)", "Daft Punk", 201);
        assert!(score(&normal, &t).is_some());
        assert_eq!(
            score(&sped, &t),
            None,
            "a sped-up version has the wrong length and must not be used"
        );
    }

    #[test]
    fn a_sped_up_version_of_the_right_length_is_penalised() {
        let t = track("One More Time", "Daft Punk", 322_000);
        let normal = video("Daft Punk - One More Time", "Daft Punk", 322);
        let sped = video("Daft Punk - One More Time (Sped Up)", "Daft Punk", 322);
        assert!(score(&normal, &t).unwrap() > score(&sped, &t).unwrap());
    }

    #[test]
    fn a_track_without_a_duration_cannot_be_matched() {
        let t = track("One More Time", "Daft Punk", 0);
        assert_eq!(
            score(&video("Daft Punk - One More Time", "Daft Punk", 322), &t),
            None
        );
    }

    #[test]
    fn parses_a_search_reply() {
        let data = json!({
            "contents": { "twoColumnSearchResultsRenderer": { "primaryContents": { "sectionListRenderer": {
                "contents": [ { "itemSectionRenderer": { "contents": [
                    { "videoRenderer": { "videoId": "abc123", "title": { "runs": [ { "text": "Song" } ] },
                                         "ownerText": { "runs": [ { "text": "Artist" } ] },
                                         "lengthSeconds": "200" } }
                ] } } ]
            } } } }
        });
        let videos = parse_search_results(&data);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].id, "abc123");
        assert_eq!(videos[0].title, "Song");
        assert_eq!(videos[0].duration_seconds, Some(200));
    }

    #[test]
    fn default_settings_keep_youtube_off() {
        let settings = YouTubeSettings::default();
        assert!(!settings.enabled);
        assert!(settings.invidious_instance.is_none());
        assert!(settings.yt_dlp_path.is_none());
    }
}
