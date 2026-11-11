use std::path::{Path, PathBuf};

/// A local audio file.
#[derive(Debug, Clone)]
pub struct LocalTrack {
    /// The full path of the file.
    pub path: PathBuf,
    /// The file name without its extension.
    pub title: String,
    /// The artist, read from the file's metadata when it carries one.
    pub artist: String,
    /// The album, read from the file's metadata when it carries one.
    pub album: String,
    /// The duration in milliseconds, when it could be determined.
    pub duration_ms: u32,
}

/// The file extensions Rustify recognises.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "oga", "opus", "m4a", "aac", "wma", "aiff", "aif", "webm",
];

/// Whether a path looks like an audio file.
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
        .unwrap_or(false)
}

/// The title of a local file: its name without the extension.
pub fn title_of(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Unknown")
        .to_string()
}

/// Read the tags of an audio file.
///
/// Only the tags that are cheap to read are used: the file name always
/// provides a title, and the folder name is used as a fallback artist.
pub fn read_tags(path: &Path) -> (String, String) {
    let title = title_of(path);
    let artist = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("Unknown")
        .to_string();
    (title, artist)
}

/// Scan a folder for audio files.
///
/// The scan is shallow: only the given folder is read, not its subfolders.
pub fn scan_folder(folder: &Path) -> Vec<LocalTrack> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };

    let mut tracks: Vec<LocalTrack> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() || !is_audio_file(&path) {
                return None;
            }
            let (title, artist) = read_tags(&path);
            Some(LocalTrack {
                duration_ms: 0,
                path,
                title,
                artist,
                album: String::new(),
            })
        })
        .collect();

    tracks.sort_by_key(|track| track.title.to_lowercase());
    tracks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_audio_extensions() {
        assert!(is_audio_file(Path::new("/music/song.mp3")));
        assert!(is_audio_file(Path::new("/music/song.FLAC")));
        assert!(is_audio_file(Path::new("/music/song.opus")));
        assert!(!is_audio_file(Path::new("/music/cover.jpg")));
        assert!(!is_audio_file(Path::new("/music/song")));
    }

    #[test]
    fn derives_a_title_from_the_file_name() {
        assert_eq!(title_of(Path::new("/music/Some Song.mp3")), "Some Song");
    }

    #[test]
    fn uses_the_folder_as_the_artist_fallback() {
        let (title, artist) = read_tags(Path::new("/music/Artist/Track.mp3"));
        assert_eq!(title, "Track");
        assert_eq!(artist, "Artist");
    }

    #[test]
    fn scans_a_folder_and_ignores_other_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.mp3"), b"x").unwrap();
        std::fs::write(dir.path().join("a.flac"), b"x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();

        let tracks = scan_folder(dir.path());
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "a");
        assert_eq!(tracks[1].title, "b");
    }

    #[test]
    fn scanning_a_missing_folder_is_empty() {
        assert!(scan_folder(Path::new("/definitely/not/here")).is_empty());
    }
}
