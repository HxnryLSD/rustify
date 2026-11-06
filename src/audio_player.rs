use crate::ad_blocker::AdBlocker;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Audio player using rodio. Wraps a rodio Sink for play/pause/stop/volume.
pub struct AudioPlayer {
    _stream: OutputStream,
    sink: Sink,
    ad_blocker: Arc<Mutex<AdBlocker>>,
    volume: Arc<Mutex<f32>>,
}

impl AudioPlayer {
    pub fn new(ad_blocker: AdBlocker) -> Result<Self, Box<dyn std::error::Error>> {
        let (_stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        Ok(Self {
            _stream,
            sink,
            ad_blocker: Arc::new(Mutex::new(ad_blocker)),
            volume: Arc::new(Mutex::new(0.8)),
        })
    }

    /// Play an audio file from disk (e.g. a preview/MP3).
    pub fn play_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let source = Decoder::new(reader)?;
        self.sink.append(source);
        self.sink.play();
        Ok(())
    }

    /// Stop playback and clear the queue.
    pub fn stop(&self) {
        self.sink.stop();
    }

    /// Pause playback.
    pub fn pause(&self) {
        self.sink.pause();
    }

    /// Resume playback.
    pub fn resume(&self) {
        self.sink.play();
    }

    /// Set volume (0.0 to 1.0).
    pub async fn set_volume(&self, volume: f32) {
        *self.volume.lock().await = volume;
        self.sink.set_volume(volume);
    }

    /// Get current volume.
    pub async fn volume(&self) -> f32 {
        *self.volume.lock().await
    }

    /// Check if currently playing.
    pub fn is_playing(&self) -> bool {
        !self.sink.empty()
    }

    /// Vet a track against the ad blocker before playback. Returns true if safe to play.
    pub async fn is_track_allowed(&self, track_name: &str, artist_name: &str) -> bool {
        let blocker = self.ad_blocker.lock().await;
        !blocker.is_ad_track(track_name, artist_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ad_blocker_filters_ad_track() {
        let blocker = AdBlocker::default();
        assert!(blocker.is_ad_track("Spotify Ad", "Spotify"));
        assert!(!blocker.is_ad_track("Real Song", "Real Artist"));
    }
}
