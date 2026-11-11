use rodio::{Decoder, Player, Source};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Shared state describing what the player is currently doing.
#[derive(Default)]
struct PlayerState {
    /// Identifier of the track being played.
    track_id: Option<String>,
    /// Title shown in the UI.
    title: String,
    /// Artist shown in the UI.
    artist: String,
    /// Album art URL shown in the UI.
    art_url: Option<String>,
    /// Track duration in milliseconds.
    duration_ms: u32,
}

/// Local audio playback.
///
/// Decodes an audio stream and plays it through the default output device.
/// The player owns a rodio [`Player`] and keeps its own state so the UI can
/// render what is playing without asking the network.
pub struct AudioPlayer {
    /// Keeps the mixer alive for as long as the player exists.
    _sink: rodio::MixerDeviceSink,
    player: Player,
    state: Arc<Mutex<PlayerState>>,
    volume: Arc<Mutex<f32>>,
    playing: Arc<AtomicBool>,
}

impl AudioPlayer {
    /// Open the default audio output device.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut sink = rodio::DeviceSinkBuilder::open_default_sink()?;
        sink.log_on_drop(false);
        let player = Player::connect_new(sink.mixer());
        Ok(Self {
            _sink: sink,
            player,
            state: Arc::new(Mutex::new(PlayerState::default())),
            volume: Arc::new(Mutex::new(0.8)),
            playing: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Play an audio stream from an in-memory buffer.
    ///
    /// Spotify serves previews as MP3, so that format is tried first; WAV and
    /// the remaining common formats are used as fallbacks.
    pub fn play_bytes(
        &self,
        data: Vec<u8>,
        track_id: String,
        title: String,
        artist: String,
        art_url: Option<String>,
        duration_ms: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if data.is_empty() {
            return Err("empty audio buffer".into());
        }
        let source = decode_audio(&data)?;
        self.play_source(source, track_id, title, artist, art_url, duration_ms);
        Ok(())
    }

    /// Play an already-decoded source.
    pub fn play_source<S: Source<Item = f32> + Send + 'static>(
        &self,
        source: S,
        track_id: String,
        title: String,
        artist: String,
        art_url: Option<String>,
        duration_ms: u32,
    ) {
        self.player.clear();
        self.player.append(source);
        self.player.play();

        {
            let mut state = self.state.lock().unwrap();
            state.track_id = Some(track_id);
            state.title = title;
            state.artist = artist;
            state.art_url = art_url;
            state.duration_ms = duration_ms;
        }
        self.playing.store(true, Ordering::Relaxed);
    }

    /// Stop playback and clear the queue.
    pub fn stop(&self) {
        self.player.stop();
        self.player.clear();
        self.playing.store(false, Ordering::Relaxed);
        let mut state = self.state.lock().unwrap();
        state.track_id = None;
        state.duration_ms = 0;
    }

    /// Pause playback.
    pub fn pause(&self) {
        self.player.pause();
        self.playing.store(false, Ordering::Relaxed);
    }

    /// Resume playback.
    pub fn resume(&self) {
        if self.state.lock().unwrap().track_id.is_some() {
            self.player.play();
            self.playing.store(true, Ordering::Relaxed);
        }
    }

    /// Toggle between playing and paused.
    pub fn toggle_play(&self) {
        if self.playing.load(Ordering::Relaxed) {
            self.pause();
        } else {
            self.resume();
        }
    }

    /// Set volume (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        *self.volume.lock().unwrap() = volume;
        self.player.set_volume(volume);
    }

    /// Get the current volume.
    pub fn volume(&self) -> f32 {
        *self.volume.lock().unwrap()
    }

    /// Whether audio is currently playing.
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed) && !self.player.is_paused()
    }

    /// The elapsed playback position.
    pub fn position(&self) -> std::time::Duration {
        self.player.get_pos()
    }

    /// The identifier of the track currently loaded.
    pub fn current_track_id(&self) -> Option<String> {
        self.state.lock().unwrap().track_id.clone()
    }

    /// Title of the track currently loaded.
    pub fn current_title(&self) -> String {
        self.state.lock().unwrap().title.clone()
    }

    /// Artist of the track currently loaded.
    pub fn current_artist(&self) -> String {
        self.state.lock().unwrap().artist.clone()
    }

    /// Duration of the track currently loaded, in milliseconds.
    pub fn current_duration_ms(&self) -> u32 {
        self.state.lock().unwrap().duration_ms
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.player.stop();
    }
}

/// Decode an audio buffer, trying the formats Spotify serves.
///
/// rodio 0.23 does not sniff the container, so the format has to be named.
fn decode_audio(data: &[u8]) -> Result<Decoder<Cursor<Vec<u8>>>, Box<dyn std::error::Error>> {
    // Spotify serves previews as MP3, so that format is tried first.
    let mp3 = Decoder::new_mp3(Cursor::new(data.to_vec()));
    if let Ok(decoder) = mp3 {
        return Ok(decoder);
    }
    if let Ok(decoder) = Decoder::new_wav(Cursor::new(data.to_vec())) {
        return Ok(decoder);
    }
    if let Ok(decoder) = Decoder::new_flac(Cursor::new(data.to_vec())) {
        return Ok(decoder);
    }
    if let Ok(decoder) = Decoder::new_vorbis(Cursor::new(data.to_vec())) {
        return Ok(decoder);
    }
    Err("the audio format was not recognised".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a small mono 16-bit PCM WAV file.
    fn wav_bytes(samples: u16) -> Vec<u8> {
        let sample_rate: u32 = 8_000;
        let channels: u16 = 1;
        let bits: u16 = 16;
        let data_len = u32::from(samples) * 2;
        let byte_rate = sample_rate * u32::from(channels) * (u32::from(bits) / 8);
        let block_align = channels * bits / 8;

        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for index in 0..samples {
            let value = ((index % 32) as i16 - 16) * 1000;
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    #[test]
    fn empty_buffer_is_rejected() {
        let player = match AudioPlayer::new() {
            Ok(player) => player,
            // No audio device in this environment; the rest of the test is moot.
            Err(_) => return,
        };
        let result = player.play_bytes(
            Vec::new(),
            "id".into(),
            "title".into(),
            "artist".into(),
            None,
            1000,
        );
        assert!(result.is_err());
    }

    #[test]
    fn decodes_a_valid_wave_buffer() {
        let data = wav_bytes(4_000);
        let source = decode_audio(&data).expect("the WAV buffer decodes");
        let samples = source.count();
        assert_eq!(samples, 4_000, "every sample must be decoded");
    }

    #[test]
    fn rejects_a_buffer_that_is_not_audio() {
        let garbage = vec![0x42u8; 1_024];
        assert!(decode_audio(&garbage).is_err());
    }

    #[test]
    fn plays_a_wave_buffer_through_the_device() {
        let player = match AudioPlayer::new() {
            Ok(player) => player,
            // No audio device in this environment.
            Err(_) => return,
        };
        let data = wav_bytes(8_000);
        player
            .play_bytes(
                data,
                "track-1".into(),
                "Title".into(),
                "Artist".into(),
                None,
                1_000,
            )
            .expect("the preview plays");

        assert_eq!(player.current_track_id().as_deref(), Some("track-1"));
        assert_eq!(player.current_title(), "Title");
        assert_eq!(player.current_artist(), "Artist");
        assert_eq!(player.current_duration_ms(), 1_000);
        assert!(player.is_playing());
    }
}
