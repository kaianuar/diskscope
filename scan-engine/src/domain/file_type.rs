use std::fmt;

/// Classification of a file by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    /// Audio file (mp3, flac, wav, etc.).
    Audio,
    /// Video file (mp4, mkv, avi, etc.).
    Video,
    /// Image file (jpg, png, gif, etc.).
    Image,
    /// Document file (pdf, doc, txt, etc.).
    Document,
    /// Source code file (rs, py, js, etc.).
    Code,
    /// Archive file (zip, tar, gz, etc.).
    Archive,
    /// Unrecognized file type.
    Other,
}

impl FileType {
    /// Classify a file extension (case-insensitive).
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            // Audio
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus" | "aiff" => {
                Self::Audio
            }
            // Video
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" => {
                Self::Video
            }
            // Image
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff" | "tif"
            | "avif" | "heic" | "heif" => Self::Image,
            // Document
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp"
            | "txt" | "rtf" | "csv" | "md" => Self::Document,
            // Code
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "cpp" | "h" | "hpp" | "java"
            | "go" | "rb" | "php" | "swift" | "kt" | "scala" | "lua" | "sh" | "bash" | "zsh"
            | "css" | "scss" | "less" | "html" | "xml" | "json" | "yaml" | "yml" | "toml"
            | "sql" | "r" | "m" | "hs" | "ex" | "exs" | "erl" | "pl" | "pm" => Self::Code,
            // Archive
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" | "tbz2" | "zst" => {
                Self::Archive
            }
            _ => Self::Other,
        }
    }

    /// Classify from a filename (extracts extension first).
    pub fn from_filename(name: &str) -> Self {
        name.rsplit_once('.')
            .map(|(_, ext)| Self::from_extension(ext))
            .unwrap_or(Self::Other)
    }
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Image => "image",
            Self::Document => "document",
            Self::Code => "code",
            Self::Archive => "archive",
            Self::Other => "other",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_classify_audio_extensions() {
        assert_eq!(FileType::from_extension("mp3"), FileType::Audio);
        assert_eq!(FileType::from_extension("WAV"), FileType::Audio);
        assert_eq!(FileType::from_extension("flac"), FileType::Audio);
    }

    #[test]
    fn should_classify_video_extensions() {
        assert_eq!(FileType::from_extension("mp4"), FileType::Video);
        assert_eq!(FileType::from_extension("MKV"), FileType::Video);
    }

    #[test]
    fn should_classify_image_extensions() {
        assert_eq!(FileType::from_extension("png"), FileType::Image);
        assert_eq!(FileType::from_extension("JPG"), FileType::Image);
    }

    #[test]
    fn should_classify_document_extensions() {
        assert_eq!(FileType::from_extension("pdf"), FileType::Document);
        assert_eq!(FileType::from_extension("docx"), FileType::Document);
    }

    #[test]
    fn should_classify_code_extensions() {
        assert_eq!(FileType::from_extension("rs"), FileType::Code);
        assert_eq!(FileType::from_extension("py"), FileType::Code);
        assert_eq!(FileType::from_extension("JS"), FileType::Code);
    }

    #[test]
    fn should_classify_archive_extensions() {
        assert_eq!(FileType::from_extension("zip"), FileType::Archive);
        assert_eq!(FileType::from_extension("gz"), FileType::Archive);
    }

    #[test]
    fn should_return_other_for_unknown_extensions() {
        assert_eq!(FileType::from_extension("xyz"), FileType::Other);
        assert_eq!(FileType::from_extension(""), FileType::Other);
    }

    #[test]
    fn should_classify_from_filename() {
        assert_eq!(FileType::from_filename("song.mp3"), FileType::Audio);
        assert_eq!(FileType::from_filename("photo.JPG"), FileType::Image);
        assert_eq!(FileType::from_filename("noext"), FileType::Other);
    }
}
