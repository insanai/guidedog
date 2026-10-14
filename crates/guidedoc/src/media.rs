//! What an image node shows. As in Docutils, an image directive naming a video file
//! plays it; Guidedoc treats audio files the same way. Renderers decide from the path.

/// The media an image path names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    /// A picture.
    Image,
    /// A video file.
    Video,
    /// An audio file.
    Audio,
}

const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "m4v", "webm", "ogv", "mov"];
const AUDIO_EXTENSIONS: [&str; 8] = ["mp3", "m4a", "oga", "ogg", "opus", "wav", "flac", "aac"];

/// Reads a path's extension, ignoring case, a query, and a fragment.
pub fn media_kind(path: &str) -> MediaKind {
    let end = path.find(['?', '#']).unwrap_or(path.len());
    let path = &path[..end];
    let name = path.rsplit('/').next().unwrap_or(path);
    let Some(dot) = name.rfind('.') else {
        return MediaKind::Image;
    };
    let extension = &name[dot + 1..];
    if VIDEO_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(extension)) {
        return MediaKind::Video;
    }
    if AUDIO_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(extension)) {
        return MediaKind::Audio;
    }
    MediaKind::Image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_kinds_follow_extensions() {
        assert_eq!(media_kind("clip.MP4?x=1#t"), MediaKind::Video);
        assert_eq!(media_kind("a/b.flac"), MediaKind::Audio);
        assert_eq!(media_kind("dir.mp4/picture"), MediaKind::Image);
        assert_eq!(media_kind("logo.png"), MediaKind::Image);
    }
}
