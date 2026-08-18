mod filter;
mod node;
mod tree;

pub use filter::{Filter, FilterSet};
pub use node::{FileNode, NodeKind};
pub use tree::FileTree;

/// Classification of files by their media/usage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Audio,
    Video,
    Image,
    Document,
    Code,
    Archive,
    Other,
}

impl FileType {
    /// Classify a file by its extension (case-insensitive, without leading dot).
    pub fn from_extension(ext: &str) -> Self {
        // Fast path: ASCII lowercase without allocating for common short extensions.
        let lower = ext.to_ascii_lowercase();
        match lower.as_str() {
            // Audio
            "mp3" | "flac" | "wav" | "aac" | "ogg" | "wma" | "m4a" | "aiff" | "alac"
            | "opus" | "mid" | "midi" => Self::Audio,

            // Video
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg"
            | "mpeg" | "3gp" | "ogv" | "vob" => Self::Video,

            // Image
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff"
            | "tif" | "psd" | "raw" | "cr2" | "nef" | "heic" | "heif" | "avif" | "jxl" => {
                Self::Image
            }

            // Document
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods"
            | "odp" | "rtf" | "txt" | "csv" | "md" | "markdown" | "epub" | "mobi"
            | "pages" | "numbers" | "keynote" | "tex" | "bib" => Self::Document,

            // Code
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "go" | "c" | "cpp" | "cc"
            | "cxx" | "h" | "hpp" | "hxx" | "java" | "kt" | "kts" | "swift" | "m"
            | "mm" | "rb" | "php" | "pl" | "pm" | "lua" | "r" | "R" | "scala" | "clj"
            | "cljs" | "hs" | "ex" | "exs" | "erl" | "hrl" | "ml" | "mli" | "fs"
            | "fsx" | "zig" | "nim" | "v" | "dart" | "elm" | "jl" | "cr" | "d" | "pas"
            | "pp" | "asm" | "s" | "sol" | "move" | "toml" | "yaml" | "yml" | "json"
            | "xml" | "html" | "htm" | "css" | "scss" | "sass" | "less" | "sql" | "sh"
            | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd" | "makefile" | "cmake"
            | "dockerfile" | "proto" | "graphql" | "gql" | "tf" | "hcl" | "nix" => {
                Self::Code
            }

            // Archive
            "zip" | "tar" | "gz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "tgz"
            | "tbz2" | "txz" | "lz" | "lzma" | "sz" | "cab" | "iso" | "dmg" | "img"
            | "wim" | "swm" | "esd" | "apk" | "ipa" | "deb" | "rpm" | "msi" | "jar"
            | "war" | "ear" | "whl" | "gem" | "nupkg" | "snap" | "flatpak" => {
                Self::Archive
            }

            _ => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_classify_audio_extensions() {
        assert_eq!(FileType::from_extension("mp3"), FileType::Audio);
        assert_eq!(FileType::from_extension("flac"), FileType::Audio);
        assert_eq!(FileType::from_extension("wav"), FileType::Audio);
        assert_eq!(FileType::from_extension("aac"), FileType::Audio);
        assert_eq!(FileType::from_extension("ogg"), FileType::Audio);
        assert_eq!(FileType::from_extension("m4a"), FileType::Audio);
        assert_eq!(FileType::from_extension("opus"), FileType::Audio);
    }

    #[test]
    fn should_classify_video_extensions() {
        assert_eq!(FileType::from_extension("mp4"), FileType::Video);
        assert_eq!(FileType::from_extension("mkv"), FileType::Video);
        assert_eq!(FileType::from_extension("avi"), FileType::Video);
        assert_eq!(FileType::from_extension("mov"), FileType::Video);
        assert_eq!(FileType::from_extension("webm"), FileType::Video);
        assert_eq!(FileType::from_extension("flv"), FileType::Video);
    }

    #[test]
    fn should_classify_image_extensions() {
        assert_eq!(FileType::from_extension("jpg"), FileType::Image);
        assert_eq!(FileType::from_extension("png"), FileType::Image);
        assert_eq!(FileType::from_extension("gif"), FileType::Image);
        assert_eq!(FileType::from_extension("svg"), FileType::Image);
        assert_eq!(FileType::from_extension("webp"), FileType::Image);
        assert_eq!(FileType::from_extension("heic"), FileType::Image);
        assert_eq!(FileType::from_extension("avif"), FileType::Image);
    }

    #[test]
    fn should_classify_document_extensions() {
        assert_eq!(FileType::from_extension("pdf"), FileType::Document);
        assert_eq!(FileType::from_extension("docx"), FileType::Document);
        assert_eq!(FileType::from_extension("xlsx"), FileType::Document);
        assert_eq!(FileType::from_extension("txt"), FileType::Document);
        assert_eq!(FileType::from_extension("md"), FileType::Document);
        assert_eq!(FileType::from_extension("csv"), FileType::Document);
        assert_eq!(FileType::from_extension("epub"), FileType::Document);
    }

    #[test]
    fn should_classify_code_extensions() {
        assert_eq!(FileType::from_extension("rs"), FileType::Code);
        assert_eq!(FileType::from_extension("py"), FileType::Code);
        assert_eq!(FileType::from_extension("js"), FileType::Code);
        assert_eq!(FileType::from_extension("ts"), FileType::Code);
        assert_eq!(FileType::from_extension("go"), FileType::Code);
        assert_eq!(FileType::from_extension("java"), FileType::Code);
        assert_eq!(FileType::from_extension("json"), FileType::Code);
        assert_eq!(FileType::from_extension("yaml"), FileType::Code);
        assert_eq!(FileType::from_extension("toml"), FileType::Code);
        assert_eq!(FileType::from_extension("html"), FileType::Code);
        assert_eq!(FileType::from_extension("css"), FileType::Code);
        assert_eq!(FileType::from_extension("sh"), FileType::Code);
    }

    #[test]
    fn should_classify_archive_extensions() {
        assert_eq!(FileType::from_extension("zip"), FileType::Archive);
        assert_eq!(FileType::from_extension("tar"), FileType::Archive);
        assert_eq!(FileType::from_extension("gz"), FileType::Archive);
        assert_eq!(FileType::from_extension("7z"), FileType::Archive);
        assert_eq!(FileType::from_extension("rar"), FileType::Archive);
        assert_eq!(FileType::from_extension("deb"), FileType::Archive);
        assert_eq!(FileType::from_extension("rpm"), FileType::Archive);
        assert_eq!(FileType::from_extension("iso"), FileType::Archive);
    }

    #[test]
    fn should_return_other_for_unknown_extension() {
        assert_eq!(FileType::from_extension("xyz"), FileType::Other);
        assert_eq!(FileType::from_extension("foo"), FileType::Other);
        assert_eq!(FileType::from_extension(""), FileType::Other);
    }

    #[test]
    fn should_handle_case_insensitive_extensions() {
        assert_eq!(FileType::from_extension("MP3"), FileType::Audio);
        assert_eq!(FileType::from_extension("Mp4"), FileType::Video);
        assert_eq!(FileType::from_extension("JPG"), FileType::Image);
        assert_eq!(FileType::from_extension("PDF"), FileType::Document);
        assert_eq!(FileType::from_extension("RS"), FileType::Code);
        assert_eq!(FileType::from_extension("ZIP"), FileType::Archive);
        assert_eq!(FileType::from_extension("Py"), FileType::Code);
        assert_eq!(FileType::from_extension("JSON"), FileType::Code);
    }
}
